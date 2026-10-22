use crate::kiwix::KiwixClient;
use json::JsonValue;

pub struct McpServer {
    pub client: KiwixClient,
}

impl McpServer {
    pub fn new(client: KiwixClient) -> Self {
        Self { client }
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
            }
        }
    }

    fn handle_tools_list(&self) -> JsonValue {
        let tools = json::array![
            json::object! {
                "name" => "kiwix_list_books",
                "description" => "List all available books/ZIM files hosted on the Kiwix server, including their IDs, titles, languages, and article counts.",
                "inputSchema" => json::object! {
                    "type" => "object",
                    "properties" => json::object! {}
                }
            },
            json::object! {
                "name" => "kiwix_suggest",
                "description" => "Get search suggestions/completions for a term from Kiwix.",
                "inputSchema" => json::object! {
                    "type" => "object",
                    "properties" => json::object! {
                        "query" => json::object! {
                            "type" => "string",
                            "description" => "Prefix or search term to get suggestions for"
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
                "name" => "kiwix_search",
                "description" => "Search for articles matching a query across books or within a specific book in Kiwix.",
                "inputSchema" => json::object! {
                    "type" => "object",
                    "properties" => json::object! {
                        "query" => json::object! {
                            "type" => "string",
                            "description" => "Search terms or keywords"
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
                "name" => "kiwix_read_page",
                "description" => "Read an article or page from a Kiwix book. Returns clean, formatted Markdown by default.",
                "inputSchema" => json::object! {
                    "type" => "object",
                    "properties" => json::object! {
                        "book" => json::object! {
                            "type" => "string",
                            "description" => "Book ID or name (e.g. from kiwix_list_books or search results)"
                        },
                        "path" => json::object! {
                            "type" => "string",
                            "description" => "Article title or path (e.g. 'Philosophy' or 'A/Albert_Einstein.html')"
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
                    "required" => json::array!["book", "path"]
                }
            },
            json::object! {
                "name" => "kiwix_random",
                "description" => "Retrieve a random article from a Kiwix book, formatted as Markdown.",
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
                "name" => "kiwix_status",
                "description" => "Check Kiwix server connectivity, latency, and available books count.",
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
        let name = params["name"].as_str().ok_or((-32602, "Missing tool name".to_string()))?;
        let args = &params["arguments"];

        let call_res = match name {
            "kiwix_list_books" => self.tool_list_books(),
            "kiwix_suggest" => self.tool_suggest(args),
            "kiwix_search" => self.tool_search(args),
            "kiwix_read_page" => self.tool_read_page(args),
            "kiwix_random" => self.tool_random(args),
            "kiwix_status" => self.tool_status(),
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
        let books = self.client.list_books()?;
        let mut arr = JsonValue::new_array();
        for b in &books {
            let _ = arr.push(b.to_json());
        }
        Ok(arr.pretty(2))
    }

    fn tool_suggest(&self, args: &JsonValue) -> Result<String, String> {
        let query = args["query"].as_str().ok_or("Missing required parameter 'query'")?;
        let book = args["book"].as_str();
        let limit = args["limit"].as_usize();

        let suggestions = self.client.suggest(query, book, limit)?;
        Ok(suggestions.pretty(2))
    }

    fn tool_search(&self, args: &JsonValue) -> Result<String, String> {
        let query = args["query"].as_str().ok_or("Missing required parameter 'query'")?;
        let book = args["book"].as_str();
        let count = args["count"].as_usize();

        let results = self.client.search(query, book, count)?;
        let mut arr = JsonValue::new_array();
        for r in results {
            let _ = arr.push(r.to_json());
        }
        Ok(arr.pretty(2))
    }

    fn tool_read_page(&self, args: &JsonValue) -> Result<String, String> {
        let book = args["book"].as_str().ok_or("Missing required parameter 'book'")?;
        let path = args["path"].as_str().ok_or("Missing required parameter 'path'")?;
        let raw_html = args["raw_html"].as_bool().unwrap_or(false);
        let max_length = args["max_length"].as_usize();

        self.client.read_page(book, path, raw_html, max_length)
    }

    fn tool_random(&self, args: &JsonValue) -> Result<String, String> {
        let book = args["book"].as_str();
        let article = self.client.random_page(book)?;
        Ok(article.pretty(2))
    }

    fn tool_status(&self) -> Result<String, String> {
        let status = self.client.server_status()?;
        Ok(status.pretty(2))
    }
}
