pub mod html;
pub mod http;
pub mod kiwix;
pub mod mcp;

// Backwards-compatible aliases for previous module paths
pub use html as html_to_md;
pub use http as http_client;

pub use html::{decode_html_entities, html_to_markdown};
pub use http::{HttpClient, HttpResponse, SimpleUrl};
pub use kiwix::{KiwixBook, KiwixClient, KiwixSearchResult};
pub use mcp::McpServer;
