use std::path::Path;
use std::sync::Arc;
use zodiac::{McpServer, ZimArchive, ZimLibrary};

const TEST_ZIM_PATH: &str =
    r"D:\Applications\Offline Wikipedia\Wikipedia\wikipedia_fa_all_nopic_2026-01.zim";

#[test]
fn test_embedded_zim_archive() {
    if !Path::new(TEST_ZIM_PATH).exists() {
        eprintln!(
            "Test ZIM file not found at {}, skipping test.",
            TEST_ZIM_PATH
        );
        return;
    }

    let archive = ZimArchive::open(TEST_ZIM_PATH).expect("Failed to open ZIM archive");
    assert_eq!(archive.info.title, "ویکی‌پدیا");
    assert!(archive.info.article_count > 1_000_000);

    // Test suggest
    let suggestions = archive.suggest("ایران", 5).expect("Suggest failed");
    assert!(!suggestions.is_empty(), "Expected suggestions for 'ایران'");
    assert!(suggestions[0].title.contains("ایران"));

    // Test read main page
    let md = archive
        .read_page("صفحهٔ_اصلی", false, Some(5000))
        .expect("Read page failed");
    assert!(!md.is_empty(), "Expected non-empty markdown for main page");
    assert!(md.contains("صفحهٔ اصلی"));

    // Test search
    let search_res = archive.search("ایران", 3).expect("Search failed");
    assert!(!search_res.is_empty(), "Expected search results");
    assert!(search_res[0].title.contains("ایران"));

    // Test random
    let rnd = archive.random_page().expect("Random page failed");
    assert!(!rnd["title"].is_null());
}

#[test]
fn test_mcp_server_protocol() {
    if !Path::new(TEST_ZIM_PATH).exists() {
        eprintln!("Test ZIM file not found, skipping MCP test.");
        return;
    }

    let mut library = ZimLibrary::new();
    library
        .load_file(TEST_ZIM_PATH)
        .expect("Failed to load archive");
    let server = McpServer::new(Arc::new(library));

    // 1. initialize
    let init_resp = server
        .handle_message(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#)
        .unwrap();
    let init_json = json::parse(&init_resp).unwrap();
    assert_eq!(init_json["result"]["serverInfo"]["name"], "zodiac");

    // 2. tools/list
    let list_resp = server
        .handle_message(r#"{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}"#)
        .unwrap();
    let list_json = json::parse(&list_resp).unwrap();
    let tools = &list_json["result"]["tools"];
    assert!(tools.members().any(|t| t["name"] == "zim_read_page"));
    assert!(tools.members().any(|t| t["name"] == "zim_suggest"));
    assert!(tools.members().any(|t| t["name"] == "zim_search"));
    assert!(tools.members().any(|t| t["name"] == "zim_list_books"));

    // 3. tools/call zim_list_books
    let call_books = server.handle_message(r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"zim_list_books","arguments":{}}}"#).unwrap();
    let books_json = json::parse(&call_books).unwrap();
    assert_eq!(books_json["result"]["isError"], false);

    // 4. tools/call zim_suggest
    let call_sugg = server.handle_message(r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"zim_suggest","arguments":{"query":"ایران","limit":3}}}"#).unwrap();
    let sugg_json = json::parse(&call_sugg).unwrap();
    assert_eq!(sugg_json["result"]["isError"], false);

    // 5. tools/call zim_read_page
    let call_read = server.handle_message(r#"{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"zim_read_page","arguments":{"path":"صفحهٔ_اصلی"}}}"#).unwrap();
    let read_json = json::parse(&call_read).unwrap();
    assert_eq!(read_json["result"]["isError"], false);

    // 6. tools/call zim_status
    let call_status = server.handle_message(r#"{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"zim_status","arguments":{}}}"#).unwrap();
    let status_json = json::parse(&call_status).unwrap();
    assert_eq!(status_json["result"]["isError"], false);
}
