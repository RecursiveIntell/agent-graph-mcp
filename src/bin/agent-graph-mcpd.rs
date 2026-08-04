use agent_graph_mcp::{cli, daemon, AgentGraphServer};
use rmcp::ServiceExt;
use std::{os::unix::fs::PermissionsExt, path::PathBuf};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing. Set RUST_LOG=debug for verbose daemon logs.
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();

    let mut data = PathBuf::from("/tmp/agent-graph");
    let mut socket = PathBuf::from("/tmp/agent-graph/mcp.sock");
    let mut base_url = String::from("http://127.0.0.1:11434");
    let mut model = String::from("glm-5.2:cloud");
    let mut max_graphs: usize = 64;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--data-dir" => data = PathBuf::from(args.next().ok_or("missing data dir")?),
            "--socket" => socket = PathBuf::from(args.next().ok_or("missing socket")?),
            "--base-url" => base_url = args.next().ok_or("missing base URL")?,
            "--model" => model = args.next().ok_or("missing model name")?,
            "--max-graphs" => {
                max_graphs = args
                    .next()
                    .ok_or("missing max-graphs value")?
                    .parse()
                    .map_err(|_| "max-graphs must be a number")?;
            }
            "--help" => {
                println!(
                    "agent-graph-mcpd --data-dir PATH --socket PATH [--base-url URL] [--model NAME] [--max-graphs N]"
                );
                return Ok(());
            }
            "--version" => {
                println!("{}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            _ => return Err("unknown daemon argument".into()),
        }
    }
    std::fs::create_dir_all(&data)?;
    let key_path = std::env::var_os("AGENT_GRAPH_INTEGRITY_KEY_PATH").map(PathBuf::from);
    let _store = agent_graph_mcp::store::PersistentStore::open_with_integrity_key(
        &data,
        key_path.as_deref(),
    )
    .map_err(std::io::Error::other)?;
    let (_lock, conn) = daemon::open_owned(&data, "daemon")?;
    daemon::enforce_startup_mode(&conn, key_path.is_some())
        .map_err(|e| format!("daemon startup rejected: {e}"))?;
    let id = daemon::identity(&conn)?;
    let _ = daemon::recover_owned_state(&conn, &id.instance_id, id.generation)?;
    drop(conn);
    let rt = tokio::runtime::Runtime::new()?;
    let data_dir = data.clone();
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
                let base_url = base_url.clone();
                let model = model.clone();
                tokio::spawn(async move {
                    let _ = serve_connection(stream, &data_dir, key_path.as_deref(), &base_url, &model, max_graphs)
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
    base_url: &str,
    model: &str,
    max_graphs: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let (mut sock_rx, mut sock_tx) = stream.into_split();
    let (bridge_side, rmcp_side) = tokio::io::duplex(1024 * 1024 + 4096);
    let (bridge_rx, mut bridge_tx) = tokio::io::split(bridge_side);

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

    let server = AgentGraphServer::new_with_max_graphs(
        base_url.to_owned(),
        model.to_owned(),
        Some(data_dir.to_path_buf()),
        key_path.map(|p| p.to_path_buf()),
        max_graphs,
    )
    .map_err(std::io::Error::other)?;

    let service = match server.serve(rmcp_side).await {
        Ok(service) => service,
        Err(_) => {
            to_rmcp.abort();
            from_rmcp.abort();
            return Ok(());
        }
    };
    let _ = service.waiting().await;
    to_rmcp.abort();
    from_rmcp.abort();

    Ok(())
}
