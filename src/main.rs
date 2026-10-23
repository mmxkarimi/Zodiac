use std::env;
use std::io::{self, BufRead, Write};
use std::path::Path;
use std::sync::Arc;
use zodiac::{McpServer, ZimLibrary};

fn print_help() {
    eprintln!(
        "Zodiac - Embedded ZIM Reader & Model Context Protocol (MCP) Server\n\n\
        Usage: zodiac [OPTIONS] [ZIM_FILES...]\n\n\
        Options:\n\
          -z, --zim <PATH>    Path to a .zim file (can be specified multiple times)\n\
          -h, --help          Print help information\n\
          -v, --version       Print version information\n\n\
        Environment:\n\
          ZIM_PATH            Semicolon-separated (or colon-separated) paths to .zim files\n\n\
        Transport:\n\
          Runs as an MCP server over standard input/output (stdio).\n"
    );
}

fn main() {
    let mut zim_paths: Vec<String> = Vec::new();

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
            "-z" | "--zim" => {
                if i + 1 < args.len() {
                    zim_paths.push(args[i + 1].clone());
                    i += 1;
                } else {
                    eprintln!("[ERROR] --zim requires a file path argument");
                    std::process::exit(1);
                }
            }
            other => {
                if !other.starts_with('-') {
                    zim_paths.push(other.to_string());
                } else {
                    eprintln!("[WARN] Unknown option: {}", other);
                }
            }
        }
        i += 1;
    }

    // Fallback to ZIM_PATH environment variable if no paths provided via CLI
    if zim_paths.is_empty() {
        if let Ok(env_path) = env::var("ZIM_PATH") {
            let separator = if cfg!(windows) { ';' } else { ':' };
            for part in env_path.split(separator) {
                let trimmed = part.trim();
                if !trimmed.is_empty() {
                    zim_paths.push(trimmed.to_string());
                }
            }
        }
    }

    if zim_paths.is_empty() {
        eprintln!("[ERROR] No ZIM files specified.");
        eprintln!("[INFO] Usage: zodiac --zim <path-to-file.zim>");
        eprintln!("[INFO] Or set the ZIM_PATH environment variable.");
        std::process::exit(1);
    }

    eprintln!("[INFO] Starting Zodiac MCP server (stdio transport)");

    let mut library = ZimLibrary::new();

    for p in &zim_paths {
        let path = Path::new(p);
        if !path.exists() {
            eprintln!("[ERROR] File does not exist: {}", p);
            continue;
        }

        match library.load_file(path) {
            Ok(_) => {
                let last = library.archives.last().unwrap();
                eprintln!(
                    "[INFO] Loaded '{}' ({}) - {} articles [{}]",
                    last.info.title, last.info.id, last.info.article_count, p
                );
            }
            Err(e) => {
                eprintln!("[ERROR] Failed to load '{}': {}", p, e);
            }
        }
    }

    if library.is_empty() {
        eprintln!("[FATAL] Could not load any valid ZIM archives. Exiting.");
        std::process::exit(1);
    }

    eprintln!(
        "[INFO] Zodiac ready with {} archive(s). Awaiting MCP commands on stdin...",
        library.len()
    );

    let server = McpServer::new(Arc::new(library));

    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut stdout_lock = stdout.lock();

    for line in stdin.lock().lines() {
        match line {
            Ok(line_str) => {
                let trimmed = line_str.trim().trim_start_matches('\u{feff}');
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

    eprintln!("[INFO] Zodiac MCP server stopped.");
}
