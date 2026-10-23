pub mod html;
pub mod mcp;
pub mod zim;

pub use html::{decode_html_entities, html_to_markdown};
pub use mcp::McpServer;
pub use zim::{ZimArchive, ZimBookInfo, ZimLibrary, ZimSearchResult, ZimSuggestion};
