use agent_graph_mcp::{daemon, AgentGraphServer};
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
    let mut api_key: Option<String> = None;
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
            "--api-key" => {
                api_key = Some(args.next().ok_or("missing api-key value")?.to_string());
            }
            "--help" => {
                println!(
                    "agent-graph-mcpd --data-dir PATH --socket PATH [--base-url URL] [--model NAME] [--api-key KEY (deprecated; prefer AGENT_GRAPH_API_KEY env)] [--max-graphs N]"
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
    // B1: key out of argv — explicit flag wins (deprecated), else env var.
    let api_key = daemon::resolve_api_key(api_key, std::env::var("AGENT_GRAPH_API_KEY").ok());
    // B2: shared provider-path health, probed by a daemon task below.
    let provider_health = agent_graph_mcp::provider_health::ProviderHealth::new();
    let key_path = std::env::var_os("AGENT_GRAPH_INTEGRITY_KEY_PATH").map(PathBuf::from);
    let store = agent_graph_mcp::store::PersistentStore::open_with_integrity_key(
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
            // ── Authenticated operator service (peer-credentialed Unix socket) ──
            let operator_socket = socket
                .parent()
                .map(|p| p.join("operator.sock"))
                .unwrap_or_else(|| std::path::PathBuf::from("/tmp/agent-graph/operator.sock"));
            // E1/B10: publish the well-known status file (single source of
            // truth for socket paths), then refresh it with the health probe.
            let write_status = |store: &agent_graph_mcp::store::PersistentStore,
                                mcp_socket: &std::path::Path,
                                op_socket: &std::path::Path| {
                if let Some(path) = mcp_socket.parent() {
                    let count = store.count_live_graphs().unwrap_or(0);
                    let status = serde_json::json!({
                        "daemon_pid": std::process::id(),
                        "version": env!("CARGO_PKG_VERSION"),
                        "started_at": chrono::Utc::now().to_rfc3339(),
                        "mcp_socket": mcp_socket.to_string_lossy(),
                        "operator_socket": op_socket.to_string_lossy(),
                        "graph_count": count,
                        "capacity_state": if count <= max_graphs as i64 { "within_limit" } else { "over_limit_legacy" },
                        "limits": {"graphs": max_graphs},
                    });
                    let _ = std::fs::write(
                        path.join("status.json"),
                        serde_json::to_string_pretty(&status).unwrap_or_default(),
                    );
                    let _ = std::fs::set_permissions(
                        path.join("status.json"),
                        std::fs::Permissions::from_mode(0o600),
                    );
                }
            };
            write_status(&store, &socket, &operator_socket);
            // B2: provider-path health probe (TCP + minimal HTTP GET every 60s).
            {
                let health = provider_health.clone();
                let probe_url = base_url.clone();
                let probe_store = store.clone();
                let probe_mcp = socket.clone();
                let probe_op = operator_socket.clone();
                tokio::spawn(async move {
                    loop {
                        let ok = tokio::task::spawn_blocking({
                            let url = probe_url.clone();
                            move || {
                                agent_graph_mcp::provider_health::probe_base_url(&url, 5000).is_ok()
                            }
                        })
                        .await
                        .unwrap_or(false);
                        if ok {
                            health.record_success();
                        } else {
                            health.record_failure("provider probe failed (tcp/http)");
                        }
                        // Refresh the well-known status file with the probe.
                        let status_store = probe_store.clone();
                        let status_mcp = probe_mcp.clone();
                        let status_op = probe_op.clone();
                        let _ = tokio::task::spawn_blocking(move || {
                            if let Some(path) = status_mcp.parent() {
                                let count = status_store.count_live_graphs().unwrap_or(0);
                                let status = serde_json::json!({
                                    "daemon_pid": std::process::id(),
                                    "version": env!("CARGO_PKG_VERSION"),
                                    "mcp_socket": status_mcp.to_string_lossy(),
                                    "operator_socket": status_op.to_string_lossy(),
                                    "graph_count": count,
                                    "capacity_state": if count <= max_graphs as i64 { "within_limit" } else { "over_limit_legacy" },
                                    "limits": {"graphs": max_graphs},
                                });
                                let _ = std::fs::write(
                                    path.join("status.json"),
                                    serde_json::to_string_pretty(&status).unwrap_or_default(),
                                );
                            }
                        })
                        .await;
                        tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                    }
                });
            }
            // D3: automatic retention GC — non-destructive transitions only
            // (archived / expired_pending_review), receipt-bearing, 6h cadence.
            {
                let gc_store = store.clone();
                tokio::spawn(async move {
                    loop {
                        tokio::time::sleep(std::time::Duration::from_secs(6 * 3600)).await;
                        let now = chrono::Utc::now().to_rfc3339();
                        let _ = gc_store.gc_run_policy(&now);
                    }
                });
            }
            // ── Authenticated operator service (peer-credentialed Unix socket) ──
            let operator_store = store.clone();
            let instance_id = id.instance_id.clone();
            let allowed_uids: std::collections::BTreeSet<u32> =
                std::iter::once(unsafe { libc::getuid() }).collect();
            tokio::spawn(async move {
                if operator_socket.exists() {
                    let _ = tokio::fs::remove_file(&operator_socket).await;
                }
                if let Some(p) = operator_socket.parent() {
                    let _ = tokio::fs::create_dir_all(p).await;
                }
                if let Ok(op_listener) = tokio::net::UnixListener::bind(&operator_socket) {
                    let _ = std::fs::set_permissions(
                        &operator_socket,
                        std::fs::Permissions::from_mode(0o600),
                    );
                    let service = agent_graph_mcp::operator::OperatorService::new(
                        operator_store,
                        allowed_uids,
                        instance_id,
                    );
                    while let Ok((stream, _)) = op_listener.accept().await {
                        let svc = service.clone();
                        tokio::spawn(async move {
                            let _ = agent_graph_mcp::operator::serve_connection(stream, svc).await;
                        });
                    }
                }
            });
            loop {
                let (stream, _) = match listener.accept().await {
                    Ok(pair) => pair,
                    Err(err) => break Err(Box::<dyn std::error::Error>::from(err)),
                };
                let data_dir = data_dir.clone();
                let key_path = key_path.clone();
                let base_url = base_url.clone();
                let model = model.clone();
                let api_key = api_key.clone();
                let provider_health = provider_health.clone();
                tokio::spawn(async move {
                    let _ = serve_connection(
                        stream,
                        &data_dir,
                        key_path.as_deref(),
                        &base_url,
                        &model,
                        api_key.as_deref(),
                        max_graphs,
                        provider_health,
                    )
                    .await;
                });
            }
        });
    accept_result?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn serve_connection(
    stream: tokio::net::UnixStream,
    data_dir: &std::path::Path,
    key_path: Option<&std::path::Path>,
    base_url: &str,
    model: &str,
    api_key: Option<&str>,
    max_graphs: usize,
    provider_health: agent_graph_mcp::provider_health::ProviderHealth,
) -> Result<(), Box<dyn std::error::Error>> {
    let (mut sock_rx, mut sock_tx) = stream.into_split();
    let (bridge_side, rmcp_side) = tokio::io::duplex(1024 * 1024 + 4096);
    let (bridge_rx, mut bridge_tx) = tokio::io::split(bridge_side);

    // B5: protocol hello handshake on the first frame. Legacy (non-hello)
    // frames are forwarded into the MCP bridge unchanged; version mismatch
    // replies with a hello_error frame and drops the connection (fail fast).
    {
        let mut hdr = [0u8; 4];
        if sock_rx.read_exact(&mut hdr).await.is_err() {
            return Ok(());
        }
        let len = u32::from_be_bytes(hdr) as usize;
        if len > 1024 * 1024 {
            return Ok(());
        }
        let mut payload = vec![0u8; len];
        if sock_rx.read_exact(&mut payload).await.is_err() {
            return Ok(());
        }
        match agent_graph_mcp::transport::interpret_hello(&payload) {
            Some(Ok(reply)) => {
                let _ = sock_tx.write_all(&(reply.len() as u32).to_be_bytes()).await;
                let _ = sock_tx.write_all(&reply).await;
                let _ = sock_tx.flush().await;
            }
            Some(Err(reason)) => {
                let _ = sock_tx
                    .write_all(&(reason.len() as u32).to_be_bytes())
                    .await;
                let _ = sock_tx.write_all(reason.as_bytes()).await;
                let _ = sock_tx.flush().await;
                return Ok(());
            }
            None => {
                // Legacy proxy: forward the first MCP frame into the bridge.
                let _ = bridge_tx.write_all(&payload).await;
                let _ = bridge_tx.write_all(b"\n").await;
                let _ = bridge_tx.flush().await;
            }
        }
    }

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

    let server = AgentGraphServer::new_with_max_graphs_and_key(
        base_url.to_owned(),
        model.to_owned(),
        Some(data_dir.to_path_buf()),
        key_path.map(|p| p.to_path_buf()),
        max_graphs,
        api_key.map(|s| s.to_owned()),
    )
    .map_err(std::io::Error::other)?
    .with_provider_health(provider_health);

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
