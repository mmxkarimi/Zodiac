use crate::zim::ZimLibrary;
use json::JsonValue;
use std::sync::Arc;

pub struct McpServer {
    pub library: Arc<ZimLibrary>,
}

impl McpServer {
    pub fn new(library: Arc<ZimLibrary>) -> Self {
        Self { library }
    }

    pub fn handle_message(&self, raw: &str) -> Option<String> {
        let parsed = match json::parse(raw) {
            Ok(val) => val,
            Err(e) => {
                let err_resp = json::object! {
                    "jsonrpc" => "2.0",
                    "id" => JsonValue::Null,
                    "error" => json::object! {
                        "code" => -32700,
                        "message" => format!("Parse error: {}", e)
                    }
                };
                return Some(err_resp.dump());
            }
        };

        let method = parsed["method"].as_str().unwrap_or("");
        let id = &parsed["id"];
        let is_notification = id.is_null();

        // Handle notifications
        if is_notification {
            match method {
                "notifications/initialized" => {
                    eprintln!("[INFO] MCP client initialized successfully");
                }
                "notifications/cancelled" => {
                    eprintln!("[INFO] Request cancelled by client");
                }
                _ => {
                    eprintln!("[DEBUG] Received notification: {}", method);
                }
            }
            return None;
        }

        let result = match method {
            "initialize" => Ok(self.handle_initialize(&parsed["params"])),
            "ping" => Ok(json::object! {}),
            "tools/list" => Ok(self.handle_tools_list()),
            "tools/call" => self.handle_tools_call(&parsed["params"]),
            "resources/list" => Ok(json::object! { "resources" => json::JsonValue::new_array() }),
            "prompts/list" => Ok(json::object! { "prompts" => json::JsonValue::new_array() }),
            _ => Err((-32601, format!("Method '{}' not found", method))),
        };

        let response = match result {
            Ok(res) => json::object! {
                "jsonrpc" => "2.0",
                "id" => id.clone(),
                "result" => res
            },
            Err((code, message)) => json::object! {
                "jsonrpc" => "2.0",
                "id" => id.clone(),
                "error" => json::object! {
                    "code" => code,
                    "message" => message
                }
            },
        };

        Some(response.dump())
    }

    fn handle_initialize(&self, _params: &JsonValue) -> JsonValue {
        json::object! {
            "protocolVersion" => "2024-11-05",
            "capabilities" => json::object! {
                "tools" => json::object! {}
            },
            "serverInfo" => json::object! {
                "name" => "zodiac",
                "version" => env!("CARGO_PKG_VERSION")
            },
            "instructions" => "Zodiac provides access to offline ZIM archives (such as Wikipedia). When searching or retrieving information:\n- Always search for broad, high-level concepts, main topics, or entity names (1-3 words max, e.g., 'Photosynthesis', 'Sasanian Empire', 'Quantum mechanics').\n- Never search using full sentences, complex questions, or overly detailed/granular queries (e.g., avoid 'causes of the fall of the Sasanian empire in the 7th century', search 'Fall of the Sasanian Empire' or 'Sasanian Empire' instead).\n- The archive matches against article titles. If a search returns few or no results, make the query broader and shorter.\n- After finding the main article, use `zim_read_page` to inspect the full content and extract specific details."
        }
    }

    fn handle_tools_list(&self) -> JsonValue {
        let tools = json::array![
            json::object! {
                "name" => "zim_list_books",
                "description" => "List all loaded books/ZIM files, including their IDs, titles, languages, and article counts.",
                "inputSchema" => json::object! {
                    "type" => "object",
                    "properties" => json::object! {}
                }
            },
            json::object! {
                "name" => "zim_suggest",
                "description" => "Get instant prefix search suggestions/completions for a broad term or title prefix from a ZIM archive.",
                "inputSchema" => json::object! {
                    "type" => "object",
                    "properties" => json::object! {
                        "query" => json::object! {
                            "type" => "string",
                            "description" => "Prefix or start of a broad article title (1-2 words). Keep it general and avoid overly specific phrases."
                        },
                        "book" => json::object! {
                            "type" => "string",
                            "description" => "Optional book ID or name (defaults to the first book available)"
                        },
                        "limit" => json::object! {
                            "type" => "integer",
                            "description" => "Maximum number of suggestions to return (default: 10)"
                        }
                    },
                    "required" => json::array!["query"]
                }
            },
            json::object! {
                "name" => "zim_search",
                "description" => "Search for encyclopedia articles across loaded ZIM books or within a specific book. ALWAYS search using broad, general topic names or main concepts (1-3 words) rather than overly specific queries, sentences, or questions. Search broadly to find the relevant article, then use `zim_read_page` to find specific details within the text.",
                "inputSchema" => json::object! {
                    "type" => "object",
                    "properties" => json::object! {
                        "query" => json::object! {
                            "type" => "string",
                            "description" => "Broad topic, concept, or entity name (1-3 words, e.g. 'Photosynthesis', 'Sasanian Empire', 'Albert Einstein'). Do NOT use full sentences, questions, or overly specific sub-details."
                        },
                        "book" => json::object! {
                            "type" => "string",
                            "description" => "Optional book ID or name to constrain the search"
                        },
                        "count" => json::object! {
                            "type" => "integer",
                            "description" => "Maximum number of search results to return (default: 10)"
                        }
                    },
                    "required" => json::array!["query"]
                }
            },
            json::object! {
                "name" => "zim_read_page",
                "description" => "Read an article or page from a ZIM book. Use this to read the full content of articles found via broad search to discover specific details, explanations, and answers. Returns clean, formatted Markdown by default.",
                "inputSchema" => json::object! {
                    "type" => "object",
                    "properties" => json::object! {
                        "path" => json::object! {
                            "type" => "string",
                            "description" => "Article title or path (e.g. 'Philosophy' or 'Albert_Einstein')"
                        },
                        "book" => json::object! {
                            "type" => "string",
                            "description" => "Optional book ID or name (defaults to first book)"
                        },
                        "raw_html" => json::object! {
                            "type" => "boolean",
                            "description" => "If true, return the raw HTML instead of Markdown (default: false)"
                        },
                        "max_length" => json::object! {
                            "type" => "integer",
                            "description" => "Maximum characters to return before truncation (default: 15000)"
                        }
                    },
                    "required" => json::array!["path"]
                }
            },
            json::object! {
                "name" => "zim_random",
                "description" => "Retrieve a random article from a ZIM book, formatted as Markdown.",
                "inputSchema" => json::object! {
                    "type" => "object",
                    "properties" => json::object! {
                        "book" => json::object! {
                            "type" => "string",
                            "description" => "Optional book ID or name (defaults to the first book available)"
                        }
                    }
                }
            },
            json::object! {
                "name" => "zim_status",
                "description" => "Check Zodiac embedded ZIM server status, loaded archives, and total articles.",
                "inputSchema" => json::object! {
                    "type" => "object",
                    "properties" => json::object! {}
                }
            }
        ];

        json::object! {
            "tools" => tools
        }
    }

    fn handle_tools_call(&self, params: &JsonValue) -> Result<JsonValue, (i32, String)> {
        let name = params["name"]
            .as_str()
            .ok_or((-32602, "Missing tool name".to_string()))?;
        let args = &params["arguments"];

        let call_res = match name {
            "zim_list_books" => self.tool_list_books(),
            "zim_suggest" => self.tool_suggest(args),
            "zim_search" => self.tool_search(args),
            "zim_read_page" => self.tool_read_page(args),
            "zim_random" => self.tool_random(args),
            "zim_status" => self.tool_status(),
            _ => return Err((-32601, format!("Unknown tool '{}'", name))),
        };

        match call_res {
            Ok(output_text) => Ok(json::object! {
                "content" => json::array![
                    json::object! {
                        "type" => "text",
                        "text" => output_text.as_str()
                    }
                ],
                "isError" => false
            }),
            Err(err_msg) => Ok(json::object! {
                "content" => json::array![
                    json::object! {
                        "type" => "text",
                        "text" => format!("Error executing {}: {}", name, err_msg)
                    }
                ],
                "isError" => true
            }),
        }
    }

    fn tool_list_books(&self) -> Result<String, String> {
        let books = self.library.list_books();
        let mut arr = JsonValue::new_array();
        for b in &books {
            let _ = arr.push(b.to_json());
        }
        Ok(arr.pretty(2))
    }

    fn tool_suggest(&self, args: &JsonValue) -> Result<String, String> {
        let query = args["query"]
            .as_str()
            .ok_or("Missing required parameter 'query'")?;
        let book = args["book"].as_str();
        let limit = args["limit"].as_usize();

        let suggestions = self.library.suggest(query, book, limit)?;
        Ok(suggestions.pretty(2))
    }

    fn tool_search(&self, args: &JsonValue) -> Result<String, String> {
        let query = args["query"]
            .as_str()
            .ok_or("Missing required parameter 'query'")?;
        let book = args["book"].as_str();
        let count = args["count"].as_usize();

        let results = self.library.search(query, book, count)?;
        let mut arr = JsonValue::new_array();
        for r in results {
            let _ = arr.push(r.to_json());
        }
        Ok(arr.pretty(2))
    }

    fn tool_read_page(&self, args: &JsonValue) -> Result<String, String> {
        let path = args["path"]
            .as_str()
            .ok_or("Missing required parameter 'path'")?;
        let book = args["book"].as_str().unwrap_or("");
        let raw_html = args["raw_html"].as_bool().unwrap_or(false);
        let max_length = args["max_length"].as_usize();

        let book_opt = if book.is_empty() { None } else { Some(book) };
        let archive = self.library.resolve_archive(book_opt)?;
        archive.read_page(path, raw_html, max_length)
    }

    fn tool_random(&self, args: &JsonValue) -> Result<String, String> {
        let book = args["book"].as_str();
        let article = self.library.random_page(book)?;
        Ok(article.pretty(2))
    }

    fn tool_status(&self) -> Result<String, String> {
        let status = self.library.server_status()?;
        Ok(status.pretty(2))
    }
}
