use super::models::{KiwixBook, KiwixSearchResult};
use crate::html::{decode_html_entities, html_to_markdown};
use crate::http::{url_encode, HttpClient};
use json::JsonValue;
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct KiwixClient {
    pub base_url: String,
    pub http: HttpClient,
}

impl KiwixClient {
    pub fn new(base_url: &str) -> Self {
        let trimmed = base_url.trim_end_matches('/');
        Self {
            base_url: trimmed.to_string(),
            http: HttpClient::default(),
        }
    }

    pub fn list_books(&self) -> Result<Vec<KiwixBook>, String> {
        let catalog_url = format!("{}/catalog/v2/entries", self.base_url);
        let resp = self.http.get(&catalog_url)?;
        if resp.status != 200 {
            return Err(format!(
                "Failed to fetch catalog from {}: HTTP {} {}",
                catalog_url, resp.status, resp.status_text
            ));
        }

        let xml = resp.text();
        Ok(Self::parse_opds_entries(&xml))
    }

    pub fn parse_opds_entries(xml: &str) -> Vec<KiwixBook> {
        let mut books = Vec::new();
        let mut rem = xml;

        while let Some(start_idx) = rem.find("<entry>") {
            rem = &rem[start_idx + 7..];
            let end_idx = match rem.find("</entry>") {
                Some(idx) => idx,
                None => break,
            };
            let entry_xml = &rem[..end_idx];
            rem = &rem[end_idx + 8..];

            let id = extract_xml_tag(entry_xml, "id").unwrap_or_default();
            let title = extract_xml_tag(entry_xml, "title").unwrap_or_default();
            let summary = extract_xml_tag(entry_xml, "summary").unwrap_or_default();
            let language = extract_xml_tag(entry_xml, "language").unwrap_or_default();
            let name = extract_xml_tag(entry_xml, "name").unwrap_or_default();
            let article_count = extract_xml_tag(entry_xml, "articleCount")
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(0);
            let media_count = extract_xml_tag(entry_xml, "mediaCount")
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(0);

            let content_id = extract_content_id_from_link(entry_xml).unwrap_or_else(|| name.clone());

            books.push(KiwixBook {
                id,
                content_id,
                name,
                title,
                summary,
                language,
                article_count,
                media_count,
            });
        }

        books
    }

    pub fn resolve_book(&self, book: Option<&str>) -> Result<String, String> {
        let books = self.list_books()?;
        if books.is_empty() {
            return Err("No books available on Kiwix server".to_string());
        }

        if let Some(b) = book {
            let b_trimmed = b.trim();
            if let Some(found) = books.iter().find(|bk| bk.content_id == b_trimmed) {
                return Ok(found.content_id.clone());
            }
            if let Some(found) = books.iter().find(|bk| bk.name == b_trimmed) {
                return Ok(found.content_id.clone());
            }
            if let Some(found) = books.iter().find(|bk| bk.id == b_trimmed) {
                return Ok(found.content_id.clone());
            }
            if let Some(found) = books.iter().find(|bk| bk.content_id.starts_with(b_trimmed)) {
                return Ok(found.content_id.clone());
            }
            Ok(b_trimmed.to_string())
        } else {
            Ok(books[0].content_id.clone())
        }
    }

    pub fn suggest(&self, query: &str, book: Option<&str>, limit: Option<usize>) -> Result<JsonValue, String> {
        let limit = limit.unwrap_or(10);
        let resolved_book = self.resolve_book(book)?;
        let url = format!(
            "{}/suggest?content={}&term={}&count={}",
            self.base_url,
            url_encode(&resolved_book),
            url_encode(query),
            limit
        );

        let resp = self.http.get(&url)?;
        if resp.status != 200 {
            return Err(format!("Suggest failed: HTTP {} {}", resp.status, resp.status_text));
        }

        let text = resp.text();
        let parsed = json::parse(&text).map_err(|e| format!("Invalid JSON response from suggest: {}", e))?;

        let mut results = JsonValue::new_array();

        // Kiwix suggest returns either:
        // 1) Array of objects: [{"value": "...", "label": "...", "path": "...", "kind": "path"}]
        // 2) OpenSearch array: ["term", ["sugg1", "sugg2"]]
        match parsed {
            JsonValue::Array(arr) => {
                for item in arr {
                    match item {
                        JsonValue::Object(obj) => {
                            let value = obj.get("value").and_then(|v| v.as_str()).unwrap_or("");
                            let path = obj.get("path").and_then(|v| v.as_str()).unwrap_or(value);
                            let kind = obj.get("kind").and_then(|v| v.as_str()).unwrap_or("path");
                            if kind == "path" || !value.is_empty() {
                                let _ = results.push(json::object! {
                                    "title" => value,
                                    "path" => path,
                                    "kind" => kind
                                });
                            }
                        }
                        JsonValue::String(s) => {
                            let _ = results.push(json::object! {
                                "title" => s.clone(),
                                "path" => s
                            });
                        }
                        JsonValue::Array(inner) => {
                            for sub in inner {
                                if let Some(s) = sub.as_str() {
                                    let _ = results.push(json::object! {
                                        "title" => s,
                                        "path" => s
                                    });
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => return Err("Unexpected suggest response format".to_string()),
        }

        Ok(results)
    }

    pub fn search(&self, query: &str, book: Option<&str>, count: Option<usize>) -> Result<Vec<KiwixSearchResult>, String> {
        let count = count.unwrap_or(10);
        let mut url = format!("{}/search?pattern={}&count={}", self.base_url, url_encode(query), count);
        if let Some(b) = book {
            let resolved = self.resolve_book(Some(b)).unwrap_or_else(|_| b.to_string());
            url.push_str("&content=");
            url.push_str(&url_encode(&resolved));
        }

        let resp = self.http.get(&url)?;
        if resp.status != 200 {
            return Err(format!("Search failed: HTTP {} {}", resp.status, resp.status_text));
        }

        let html = resp.text();
        Ok(Self::parse_search_html(&html))
    }

    pub fn parse_search_html(html: &str) -> Vec<KiwixSearchResult> {
        let mut results = Vec::new();

        let search_area = if let Some(pos) = html.find(r#"class="results""#) {
            &html[pos..]
        } else {
            html
        };

        let mut rem = search_area;
        while let Some(li_pos) = rem.find("<li") {
            rem = &rem[li_pos..];
            let end_li = rem.find("</li>").or_else(|| rem.find("<li")).unwrap_or(rem.len());
            let li_content = if end_li > 3 && &rem[..3] == "<li" {
                // Find next <li> or </li>
                let next_boundary = rem[3..].find("<li").map(|p| p + 3).unwrap_or(rem.len());
                let next_close = rem.find("</li>").map(|p| p + 5).unwrap_or(rem.len());
                &rem[..next_boundary.min(next_close)]
            } else {
                &rem[..end_li]
            };

            rem = &rem[li_content.len()..];

            // Extract href and title from <a href="...">Title</a>
            let (url, title) = if let Some(a_start) = li_content.find("<a ") {
                let a_rest = &li_content[a_start..];
                let href = extract_attribute(a_rest, "href").unwrap_or_default();
                let title_text = if let Some(tag_close) = a_rest.find('>') {
                    let text_rest = &a_rest[tag_close + 1..];
                    if let Some(a_end) = text_rest.find("</a>") {
                        clean_text_fragment(&text_rest[..a_end])
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                };
                (href, title_text)
            } else {
                continue;
            };

            if (url.is_empty() && title.is_empty()) || url.contains("/search?") {
                continue;
            }

            // Extract snippet from <cite>...</cite> or <p class="snippet">...</p>
            let snippet = if let Some(cite_start) = li_content.find("<cite") {
                let rest = &li_content[cite_start..];
                if let Some(tag_close) = rest.find('>') {
                    let text_rest = &rest[tag_close + 1..];
                    if let Some(cite_end) = text_rest.find("</cite>") {
                        clean_text_fragment(&text_rest[..cite_end])
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                }
            } else {
                String::new()
            };

            // Extract book title
            let book_title = if let Some(bt_start) = li_content.find(r#"class="book-title""#) {
                let rest = &li_content[bt_start..];
                if let Some(tag_close) = rest.find('>') {
                    let text_rest = &rest[tag_close + 1..];
                    if let Some(tag_end) = text_rest.find("</div>") {
                        clean_text_fragment(&text_rest[..tag_end])
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                }
            } else {
                String::new()
            };

            // Extract information (word count, etc.)
            let details = if let Some(info_start) = li_content.find(r#"class="informations""#) {
                let rest = &li_content[info_start..];
                if let Some(tag_close) = rest.find('>') {
                    let text_rest = &rest[tag_close + 1..];
                    if let Some(tag_end) = text_rest.find("</div>") {
                        clean_text_fragment(&text_rest[..tag_end])
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                }
            } else {
                String::new()
            };

            results.push(KiwixSearchResult {
                title,
                url,
                snippet,
                book_title,
                details,
            });
        }

        results
    }

    pub fn read_page(
        &self,
        book: &str,
        path: &str,
        raw_html: bool,
        max_length: Option<usize>,
    ) -> Result<String, String> {
        let max_len = max_length.unwrap_or(15000);
        let path = path.trim();
        let resolved_book = self.resolve_book(Some(book)).unwrap_or_else(|_| book.to_string());

        // Build potential URLs to try
        let urls_to_try = self.build_page_urls(&resolved_book, path);

        let mut last_error = String::new();
        for target_url in &urls_to_try {
            match self.http.get(target_url) {
                Ok(resp) => {
                    if resp.status == 200 {
                        let html = resp.text();
                        if raw_html {
                            return Ok(truncate_if_needed(&html, max_len));
                        }
                        let md = html_to_markdown(&html);
                        return Ok(truncate_if_needed(&md, max_len));
                    } else {
                        last_error = format!("HTTP {} {}", resp.status, resp.status_text);
                    }
                }
                Err(e) => {
                    last_error = e;
                }
            }
        }

        Err(format!(
            "Failed to retrieve page '{}' from book '{}'. Last error: {}. Tried: {:?}",
            path, book, last_error, urls_to_try
        ))
    }

    fn build_page_urls(&self, book: &str, path: &str) -> Vec<String> {
        let clean_path = path.trim_start_matches('/');
        let mut urls = Vec::new();

        if clean_path.starts_with("content/") {
            urls.push(format!("{}/{}", self.base_url, clean_path));
            return urls;
        }

        let encoded_path = url_encode_path(clean_path);

        // 1. Direct content url: /content/<book>/<path>
        urls.push(format!("{}/content/{}/{}", self.base_url, book, encoded_path));

        // 2. Namespace A url: /content/<book>/A/<path>
        if !clean_path.starts_with("A/") {
            urls.push(format!("{}/content/{}/A/{}", self.base_url, book, encoded_path));
        }

        // 3. Direct book url: /<book>/<path>
        urls.push(format!("{}/{}/{}", self.base_url, book, encoded_path));

        // 4. HTML extension variants if not present
        if !clean_path.ends_with(".html") && !clean_path.ends_with(".htm") {
            urls.push(format!("{}/content/{}/{}.html", self.base_url, book, encoded_path));
            if !clean_path.starts_with("A/") {
                urls.push(format!("{}/content/{}/A/{}.html", self.base_url, book, encoded_path));
            }
        }

        urls
    }

    pub fn random_page(&self, book: Option<&str>) -> Result<JsonValue, String> {
        let target_book = match book {
            Some(b) => b.to_string(),
            None => {
                let books = self.list_books()?;
                if books.is_empty() {
                    return Err("No books available on Kiwix server to pick a random page from.".to_string());
                }
                books[0].content_id.clone()
            }
        };

        let random_url = format!("{}/random?content={}", self.base_url, url_encode(&target_book));
        let resp = self.http.get(&random_url)?;

        if resp.status != 200 && (resp.status < 300 || resp.status > 399) {
            return Err(format!("Random page request failed: HTTP {} {}", resp.status, resp.status_text));
        }

        let title = extract_html_title(&resp.text()).unwrap_or_else(|| "Random Article".to_string());
        let md = html_to_markdown(&resp.text());

        Ok(json::object! {
            "book" => target_book,
            "url" => resp.final_url,
            "title" => title,
            "content" => truncate_if_needed(&md, 15000)
        })
    }

    pub fn server_status(&self) -> Result<JsonValue, String> {
        let start = Instant::now();
        let books = self.list_books()?;
        let latency_ms = start.elapsed().as_millis() as u64;

        let mut book_summaries = JsonValue::new_array();
        for b in &books {
            let _ = book_summaries.push(json::object! {
                "content_id" => b.content_id.clone(),
                "title" => b.title.clone(),
                "language" => b.language.clone(),
                "article_count" => b.article_count
            });
        }

        Ok(json::object! {
            "status" => "online",
            "base_url" => self.base_url.clone(),
            "latency_ms" => latency_ms,
            "books_count" => books.len(),
            "books" => book_summaries
        })
    }
}

fn extract_xml_tag(xml: &str, tag: &str) -> Option<String> {
    let open_tag = format!("<{}>", tag);
    let close_tag = format!("</{}>", tag);
    let start = xml.find(&open_tag)? + open_tag.len();
    let end = xml[start..].find(&close_tag)? + start;
    Some(decode_html_entities(xml[start..end].trim()))
}

fn extract_content_id_from_link(entry_xml: &str) -> Option<String> {
    let mut rem = entry_xml;
    while let Some(link_pos) = rem.find("<link ") {
        rem = &rem[link_pos..];
        let tag_end = rem.find('>').unwrap_or(rem.len());
        let tag = &rem[..tag_end];
        rem = &rem[tag_end..];

        if tag.contains(r#"type="text/html""#) {
            if let Some(href) = extract_attribute(tag, "href") {
                if let Some(idx) = href.find("/content/") {
                    let slug = &href[idx + 9..];
                    return Some(slug.trim_end_matches('/').to_string());
                }
            }
        }
    }
    None
}

fn extract_attribute(tag: &str, attr: &str) -> Option<String> {
    let pattern = format!("{}=\"", attr);
    if let Some(start) = tag.find(&pattern) {
        let val_start = start + pattern.len();
        if let Some(end) = tag[val_start..].find('"') {
            return Some(tag[val_start..val_start + end].to_string());
        }
    }
    let pattern_single = format!("{}='", attr);
    if let Some(start) = tag.find(&pattern_single) {
        let val_start = start + pattern_single.len();
        if let Some(end) = tag[val_start..].find('\'') {
            return Some(tag[val_start..val_start + end].to_string());
        }
    }
    None
}

fn clean_text_fragment(s: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for ch in s.chars() {
        if ch == '<' {
            in_tag = true;
        } else if ch == '>' {
            in_tag = false;
        } else if !in_tag {
            out.push(ch);
        }
    }
    decode_html_entities(out.trim())
}

fn extract_html_title(html: &str) -> Option<String> {
    let start = html.find("<title>")? + 7;
    let end = html[start..].find("</title>")? + start;
    Some(clean_text_fragment(&html[start..end]))
}

fn url_encode_path(path: &str) -> String {
    let mut out = String::new();
    for part in path.split('/') {
        if !out.is_empty() {
            out.push('/');
        }
        out.push_str(&url_encode(part));
    }
    out
}

fn truncate_if_needed(content: &str, max_len: usize) -> String {
    if content.chars().count() <= max_len {
        content.to_string()
    } else {
        let truncated: String = content.chars().take(max_len).collect();
        format!(
            "{}\n\n[... Truncated: showing {} characters of {}. Pass a larger max_length or refine search ...]",
            truncated,
            max_len,
            content.chars().count()
        )
    }
}
