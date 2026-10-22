use zodiac::{KiwixClient, McpServer};
use std::env;
use std::io::{self, BufRead, Write};

fn print_help() {
    eprintln!(
        "Kiwix Model Context Protocol (MCP) Server\n\n\
        Usage: zodiac [OPTIONS]\n\n\
        Options:\n\
          -u, --url <URL>     Kiwix HTTP server URL (default: KIWIX_URL env or http://127.0.0.1:8080)\n\
              --port <PORT>   Port on localhost (shortcut for http://127.0.0.1:<PORT>)\n\
          -h, --help          Print help information\n\
          -v, --version       Print version information\n\n\
        Transport:\n\
          Runs as an MCP server over standard input/output (stdio).\n"
    );
}

fn main() {
    let mut kiwix_url = env::var("KIWIX_URL").unwrap_or_else(|_| "http://127.0.0.1:8080".to_string());

    let args: Vec<String> = env::args().collect();
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-h" | "--help" => {
                print_help();
                std::process::exit(0);
            }
            "-v" | "--version" => {
                eprintln!("zodiac {}", env!("CARGO_PKG_VERSION"));
                std::process::exit(0);
            }
            "-u" | "--url" => {
                if i + 1 < args.len() {
                    kiwix_url = args[i + 1].clone();
                    i += 1;
                } else {
                    eprintln!("[ERROR] --url requires an argument");
                    std::process::exit(1);
                }
            }
            "--port" => {
                if i + 1 < args.len() {
                    let port = &args[i + 1];
                    kiwix_url = format!("http://127.0.0.1:{}", port);
                    i += 1;
                } else {
                    eprintln!("[ERROR] --port requires an argument");
                    std::process::exit(1);
                }
            }
            other => {
                if !other.starts_with('-') {
                    kiwix_url = other.to_string();
                } else {
                    eprintln!("[WARN] Unknown option: {}", other);
                }
            }
        }
        i += 1;
    }

    eprintln!("[INFO] Starting Kiwix MCP server (stdio transport)");
    eprintln!("[INFO] Target Kiwix server: {}", kiwix_url);

    let client = KiwixClient::new(&kiwix_url);

    // Initial check of server connectivity
    match client.list_books() {
        Ok(books) => {
            eprintln!(
                "[INFO] Connected to Kiwix! Found {} book(s):",
                books.len()
            );
            for b in &books {
                eprintln!(
                    "  - [{}] {} ({} articles)",
                    b.content_id, b.title, b.article_count
                );
            }
        }
        Err(e) => {
            eprintln!("[WARN] Could not reach Kiwix at {}: {}", kiwix_url, e);
            eprintln!("[WARN] Ensure kiwix-serve is running, e.g.: kiwix-serve --port=8080 <file.zim>");
        }
    }

    let server = McpServer::new(client);

    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut stdout_lock = stdout.lock();

    for line in stdin.lock().lines() {
        match line {
            Ok(line_str) => {
                let trimmed = line_str.trim();
                if trimmed.is_empty() {
                    continue;
                }
                if let Some(resp) = server.handle_message(trimmed) {
                    if let Err(e) = writeln!(stdout_lock, "{}", resp) {
                        eprintln!("[ERROR] Failed writing to stdout: {}", e);
                        break;
                    }
                    if let Err(e) = stdout_lock.flush() {
                        eprintln!("[ERROR] Failed flushing stdout: {}", e);
                        break;
                    }
                }
            }
            Err(e) => {
                eprintln!("[ERROR] Error reading from stdin: {}", e);
                break;
            }
        }
    }

    eprintln!("[INFO] Kiwix MCP server stopped.");
}
