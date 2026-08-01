use agent_graph_mcp::{cli, proxy, transport, AgentGraphServer};
use rmcp::ServiceExt;
use std::io::{self, BufRead, Write};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--direct") {
        let direct: Vec<String> = args.into_iter().skip(1).collect();
        run_direct(&direct);
        return;
    }
    let cfg = match cli::parse_proxy_args(&args) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("agent-graph-mcp: {}", e.message);
            std::process::exit(e.exit_code);
        }
    };
    let mut socket = match proxy::connect_timeout(&cfg.socket, cfg.timeout_ms) {
        Ok(s) => s,
        Err(_) => {
            eprintln!("DAEMON_UNAVAILABLE");
            std::process::exit(69);
        }
    };
    let stdin = io::stdin();
    let mut out = io::stdout();
    for line in stdin.lock().lines() {
        let line = match line {
            Ok(v) => v,
            Err(_) => break,
        };
        if transport::write_frame(&mut socket, line.as_bytes()).is_err() {
            break;
        }
        // Always read a response. For notifications (no "id"), the daemon
        // sends nothing, so the read will time out. We detect that and continue.
        // For requests (with "id"), a timeout means the daemon is slow/unreachable.
        let is_notification = line.contains("\"method\"") && !line.contains("\"id\"");
        match transport::read_frame(&mut socket) {
            Ok(response) => {
                let _ = out.write_all(&response);
                let _ = out.write_all(b"\n");
                let _ = out.flush();
            }
            Err(_) if is_notification => {
                // Expected: daemon doesn't respond to notifications.
                // Read timed out — continue to next line.
            }
            Err(_) => {
                // Unexpected read failure for a request — abort.
                break;
            }
        }
    }
}

fn run_direct(args: &[String]) {
    eprintln!("agent-graph-mcp: --direct is deprecated; use agent-graph-mcpd plus the proxy");
    let config = match cli::parse_args(args) {
        Ok(c) => c,
        Err(e) => {
            if e.exit_code != 0 {
                eprintln!("agent-graph-mcp: {}", e.message);
            }
            std::process::exit(e.exit_code);
        }
    };
    let key = config.integrity_key_path.clone().or_else(|| {
        std::env::var("AGENT_GRAPH_INTEGRITY_KEY_PATH")
            .ok()
            .map(std::path::PathBuf::from)
    });
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    rt.block_on(async move {
        let server = AgentGraphServer::new_with_max_graphs(
            config.base_url,
            config.default_model,
            config.data_dir,
            key,
            config.max_graphs,
        )
        .map_err(anyhow::Error::msg)
        .unwrap();
        let service = server.serve(rmcp::transport::stdio()).await.unwrap();
        service.waiting().await.unwrap();
    });
}
