use agent_graph_mcp::{daemon, AgentGraphServer};
use rmcp::ServiceExt;
use std::{os::unix::fs::PermissionsExt, path::PathBuf};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt};

fn validate_provider_config(base_url: &str, model: &str) -> Result<(), Box<dyn std::error::Error>> {
    if base_url != "codex-app-server://"
        && !base_url.starts_with("http://")
        && !base_url.starts_with("https://")
    {
        return Err("base URL must use http, https, or codex-app-server://".into());
    }
    if model.trim().is_empty() {
        return Err("model must not be empty".into());
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut data = PathBuf::from("/tmp/agent-graph");
    let mut socket = PathBuf::from("/tmp/agent-graph/mcp.sock");
    let mut base_url = std::env::var("AGENT_GRAPH_BASE_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:11434".to_string());
    let mut model =
        std::env::var("AGENT_GRAPH_MODEL").unwrap_or_else(|_| "glm-5.2:cloud".to_string());
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--data-dir" => data = PathBuf::from(args.next().ok_or("missing data dir")?),
            "--socket" => socket = PathBuf::from(args.next().ok_or("missing socket")?),
            "--base-url" => base_url = args.next().ok_or("missing base URL")?,
            "--model" => model = args.next().ok_or("missing model")?,
            "--help" => {
                println!("agent-graph-mcpd --data-dir PATH --socket PATH [--base-url URL] [--model NAME]");
                return Ok(());
            }
            "--version" => {
                println!("{}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            _ => return Err("unknown daemon argument".into()),
        }
    }
    validate_provider_config(&base_url, &model)?;
    std::fs::create_dir_all(&data)?;
    let key_path = std::env::var_os("AGENT_GRAPH_INTEGRITY_KEY_PATH").map(PathBuf::from);

    // The exclusive daemon lock is acquired before any SQLite open or migration.
    // This prevents a losing contender from mutating the durable store.
    let (_lock, conn) = daemon::open_owned(&data, "daemon")?;

    // Bootstrap graph/execution schema only after ownership is established, then
    // close the bootstrap connection; request servers open their own scoped store.
    let bootstrap_store = agent_graph_mcp::store::PersistentStore::open_with_integrity_key(
        &data,
        key_path.as_deref(),
    )
    .map_err(std::io::Error::other)?;
    drop(bootstrap_store);

    daemon::enforce_startup_mode(&conn, key_path.is_some())
        .map_err(|e| format!("daemon startup rejected: {e}"))?;
    let id = daemon::identity(&conn)?;
    let _ = daemon::recover_owned_state(&conn, &id.instance_id, id.generation)?;
    drop(conn);
    let rt = tokio::runtime::Runtime::new()?;
    let data_dir = data.clone();
    let provider_url = base_url.clone();
    let default_model = model.clone();
    let accept_result: std::result::Result<(), Box<dyn std::error::Error>> =
        rt.block_on(async move {
            if socket.exists() {
                tokio::fs::remove_file(&socket).await?;
            }
            if let Some(p) = socket.parent() {
                tokio::fs::create_dir_all(p).await?;
            }
            let listener = tokio::net::UnixListener::bind(&socket)?;
            std::fs::set_permissions(&socket, std::fs::Permissions::from_mode(0o600))?;
            loop {
                let (stream, _) = match listener.accept().await {
                    Ok(pair) => pair,
                    Err(err) => break Err(Box::<dyn std::error::Error>::from(err)),
                };
                let data_dir = data_dir.clone();
                let key_path = key_path.clone();
                let provider_url = provider_url.clone();
                let default_model = default_model.clone();
                tokio::spawn(async move {
                    let _ = serve_connection(
                        stream,
                        &data_dir,
                        key_path.as_deref(),
                        &provider_url,
                        &default_model,
                    )
                    .await;
                });
            }
        });
    accept_result?;
    Ok(())
}

async fn serve_connection(
    stream: tokio::net::UnixStream,
    data_dir: &std::path::Path,
    key_path: Option<&std::path::Path>,
    provider_url: &str,
    default_model: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let (mut sock_rx, mut sock_tx) = stream.into_split();

    // Create a duplex: bridge_side <-> rmcp_side
    // rmcp reads JSON-RPC lines from rmcp_side and writes responses as lines to rmcp_side
    // We bridge: decode frames -> write lines to bridge -> rmcp reads -> rmcp writes response lines -> we read and encode as frames
    let (bridge_side, rmcp_side) = tokio::io::duplex(1024 * 1024 + 4096);
    let (bridge_rx, mut bridge_tx) = tokio::io::split(bridge_side);

    // Spawn the bridges independently. Do not use `tokio::select!` here:
    // when one bridge finishes, select would cancel the response bridge while
    // rmcp is still processing the request and preparing its response.
    let to_rmcp = tokio::spawn(async move {
        loop {
            let mut hdr = [0u8; 4];
            if sock_rx.read_exact(&mut hdr).await.is_err() {
                break;
            }
            let len = u32::from_be_bytes(hdr) as usize;
            if len > 1024 * 1024 {
                break;
            }
            let mut payload = vec![0u8; len];
            if sock_rx.read_exact(&mut payload).await.is_err() {
                break;
            }
            if bridge_tx.write_all(&payload).await.is_err()
                || bridge_tx.write_all(b"\n").await.is_err()
                || bridge_tx.flush().await.is_err()
            {
                break;
            }
        }
        // Signal EOF to rmcp by closing the write side.
        drop(bridge_tx);
    });

    let from_rmcp = tokio::spawn(async move {
        let mut reader = tokio::io::BufReader::new(bridge_rx);
        loop {
            let mut line = String::new();
            match reader.read_line(&mut line).await {
                Ok(0) => break,
                Ok(_) => {
                    let trimmed = line.trim_end();
                    if trimmed.is_empty() {
                        continue;
                    }
                    let len = trimmed.len() as u32;
                    if sock_tx.write_all(&len.to_be_bytes()).await.is_err()
                        || sock_tx.write_all(trimmed.as_bytes()).await.is_err()
                        || sock_tx.flush().await.is_err()
                    {
                        break;
                    }
                    line.clear();
                }
                Err(_) => break,
            }
        }
    });

    // Create server and serve on rmcp_side of the duplex
    let server = AgentGraphServer::new(
        provider_url.to_string(),
        default_model.to_string(),
        Some(data_dir.to_path_buf()),
        key_path.map(|p| p.to_path_buf()),
    )
    .map_err(std::io::Error::other)?;

    // Keep rmcp and both bridges alive until the service finishes. Aborting
    // the bridges afterward closes the per-connection transport cleanly.
    let service = match server.serve(rmcp_side).await {
        Ok(service) => service,
        Err(_) => {
            to_rmcp.abort();
            from_rmcp.abort();
            return Ok::<(), Box<dyn std::error::Error>>(());
        }
    };
    let _ = service.waiting().await;
    to_rmcp.abort();
    from_rmcp.abort();

    Ok::<(), Box<dyn std::error::Error>>(())
}
