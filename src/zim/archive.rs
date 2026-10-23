use super::models::{ZimBookInfo, ZimSearchResult, ZimSuggestion};
use crate::html::{decode_html_entities, html_to_markdown};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;
use zim::{DirectoryEntry, Namespace, Zim};

static SEED_COUNTER: AtomicU64 = AtomicU64::new(0);

fn simple_rand(max: u32) -> u32 {
    if max == 0 {
        return 0;
    }
    let nanos = Instant::now().elapsed().as_nanos() as u64;
    let cnt = SEED_COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut state = nanos ^ (cnt.wrapping_mul(0x517cc1b727220a95));
    state ^= state >> 12;
    state ^= state << 25;
    state ^= state >> 27;
    (state.wrapping_mul(0x2545F4914F6CDD1D) % (max as u64)) as u32
}

pub struct ZimArchive {
    pub zim: Zim,
    pub info: ZimBookInfo,
    pub path: PathBuf,
}

impl ZimArchive {
    pub fn open<P: AsRef<Path>>(p: P) -> Result<Self, String> {
        let path = p.as_ref().to_path_buf();
        let zim = Zim::new(&path)
            .map_err(|e| format!("Failed to open ZIM '{}': {}", path.display(), e))?;

        let file_size_bytes = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        let file_stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("zim_archive")
            .to_string();

        let mut title = file_stem.clone();
        let mut name = file_stem.clone();
        let mut summary = String::new();
        let mut language = "und".to_string();

        if let Ok(keys) = zim.metadata_keys() {
            for k in keys {
                let val_str = match zim.metadata(&k) {
                    Ok(Some(content)) => match content.to_vec() {
                        Ok(bytes) => String::from_utf8_lossy(&bytes).trim().to_string(),
                        Err(_) => String::new(),
                    },
                    _ => String::new(),
                };

                match k.to_lowercase().as_str() {
                    "title" if !val_str.is_empty() => title = val_str,
                    "name" if !val_str.is_empty() => name = val_str,
                    "description" if !val_str.is_empty() => summary = val_str,
                    "language" if !val_str.is_empty() => language = val_str,
                    _ => {}
                }
            }
        }

        let article_count = zim.header.article_count as u64;

        let info = ZimBookInfo {
            id: file_stem,
            name,
            title,
            summary,
            language,
            article_count,
            file_path: path.to_string_lossy().to_string(),
            file_size_bytes,
        };

        Ok(Self { zim, info, path })
    }

    pub fn entry_title<'a>(entry: &'a DirectoryEntry) -> &'a str {
        if entry.title.is_empty() {
            &entry.url
        } else {
            &entry.title
        }
    }

    pub fn find_entry(&self, path: &str) -> Option<DirectoryEntry> {
        let trimmed = path.trim().trim_start_matches('/');
        let decoded = url_decode(trimmed);

        // Build candidate sub-paths
        let mut candidates = Vec::new();
        candidates.push(trimmed.to_string());
        if decoded != trimmed {
            candidates.push(decoded.clone());
        }

        // Add variants with / without namespace prefix
        for base in [trimmed.to_string(), decoded] {
            // Strip any existing C/ or A/ or -/ prefix
            if let Some(rest) = base.strip_prefix("C/").or_else(|| base.strip_prefix("A/")) {
                candidates.push(rest.to_string());
                candidates.push(rest.replace(' ', "_"));
                candidates.push(rest.replace('_', " "));
                if let Some(no_html) = rest
                    .strip_suffix(".html")
                    .or_else(|| rest.strip_suffix(".htm"))
                {
                    candidates.push(no_html.to_string());
                }
            }

            candidates.push(base.replace(' ', "_"));
            candidates.push(base.replace('_', " "));

            if let Some(no_html) = base
                .strip_suffix(".html")
                .or_else(|| base.strip_suffix(".htm"))
            {
                candidates.push(no_html.to_string());
                candidates.push(no_html.replace(' ', "_"));
            } else {
                candidates.push(format!("{}.html", base));
                candidates.push(format!("{}.html", base.replace(' ', "_")));
            }
        }

        // Search namespaces: UserContent ('C') first, then Articles ('A'), Layout ('-'), Metadata ('M')
        let namespaces = [
            Namespace::UserContent,
            Namespace::Articles,
            Namespace::Layout,
            Namespace::Metadata,
        ];

        for cand in &candidates {
            for &ns in &namespaces {
                if let Ok(Some(entry)) = self.zim.get_by_path(ns, cand) {
                    if let Ok(resolved) = self.zim.resolve(entry) {
                        return Some(resolved);
                    }
                }
            }
        }

        // Fallback: binary search in article_list_by_title
        if let Ok(Some(listing)) = self.zim.article_list_by_title() {
            if let Ok(count) = listing.len() {
                for target in [trimmed, &trimmed.replace('_', " ")] {
                    let mut low = 0;
                    let mut high = count;
                    while low < high {
                        let mid = low + (high - low) / 2;
                        if let Ok(Some(idx)) = listing.get(mid) {
                            if let Ok(entry) = self.zim.get_by_url_index(idx) {
                                let t = Self::entry_title(&entry);
                                if t < target {
                                    low = mid + 1;
                                } else {
                                    high = mid;
                                }
                            } else {
                                break;
                            }
                        } else {
                            break;
                        }
                    }

                    if low < count {
                        if let Ok(Some(idx)) = listing.get(low) {
                            if let Ok(entry) = self.zim.get_by_url_index(idx) {
                                if Self::entry_title(&entry) == target || entry.url == target {
                                    if let Ok(resolved) = self.zim.resolve(entry) {
                                        return Some(resolved);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        None
    }

    pub fn read_page(
        &self,
        path: &str,
        raw_html: bool,
        max_length: Option<usize>,
    ) -> Result<String, String> {
        let max_len = max_length.unwrap_or(15000);

        let entry = self
            .find_entry(path)
            .ok_or_else(|| format!("Article '{}' not found in book '{}'", path, self.info.title))?;

        let content = self
            .zim
            .entry_content(&entry)
            .map_err(|e| format!("Failed to read content: {}", e))?
            .ok_or_else(|| format!("Article '{}' has no content data", path))?;

        let bytes = content
            .to_vec()
            .map_err(|e| format!("Failed to decompress content: {}", e))?;

        let html = String::from_utf8_lossy(&bytes);

        if raw_html {
            Ok(truncate_if_needed(&html, max_len))
        } else {
            let md = html_to_markdown(&html);
            Ok(truncate_if_needed(&md, max_len))
        }
    }

    pub fn random_page(&self) -> Result<json::JsonValue, String> {
        let listing = self
            .zim
            .article_list_by_title()
            .map_err(|e| format!("Failed to access article listing: {}", e))?
            .or_else(|| self.zim.entry_list_by_title().ok().flatten())
            .ok_or_else(|| "No article listing available in ZIM archive".to_string())?;

        let count = listing
            .len()
            .map_err(|e| format!("Failed to get listing length: {}", e))?
            as u32;

        if count == 0 {
            return Err("ZIM archive contains no articles".to_string());
        }

        // Try up to 5 random picks until we find one with readable content
        for _ in 0..5 {
            let rnd_idx = simple_rand(count) as usize;
            if let Ok(Some(idx)) = listing.get(rnd_idx) {
                if let Ok(entry) = self.zim.get_by_url_index(idx) {
                    if let Ok(resolved) = self.zim.resolve(entry) {
                        let title = Self::entry_title(&resolved).to_string();
                        let url = resolved.url.clone();

                        if let Ok(Some(content)) = self.zim.entry_content(&resolved) {
                            if let Ok(bytes) = content.to_vec() {
                                let html = String::from_utf8_lossy(&bytes);
                                let md = html_to_markdown(&html);
                                return Ok(json::object! {
                                    "book" => self.info.id.clone(),
                                    "book_title" => self.info.title.clone(),
                                    "url" => url,
                                    "title" => title,
                                    "content" => truncate_if_needed(&md, 15000)
                                });
                            }
                        }
                    }
                }
            }
        }

        Err("Failed to pick a random article with valid content".to_string())
    }

    pub fn suggest(&self, query: &str, limit: usize) -> Result<Vec<ZimSuggestion>, String> {
        let listing = match self.zim.article_list_by_title() {
            Ok(Some(l)) => l,
            _ => match self.zim.entry_list_by_title() {
                Ok(Some(l)) => l,
                _ => return Ok(Vec::new()),
            },
        };

        let count = listing.len().map_err(|e| e.to_string())?;
        if count == 0 {
            return Ok(Vec::new());
        }

        let q_clean = query.trim();
        if q_clean.is_empty() {
            return Ok(Vec::new());
        }

        let q_lower = q_clean.to_lowercase();
        let mut results = Vec::new();

        // Binary search for prefix start
        let mut low = 0;
        let mut high = count;
        while low < high {
            let mid = low + (high - low) / 2;
            if let Ok(Some(idx)) = listing.get(mid) {
                if let Ok(entry) = self.zim.get_by_url_index(idx) {
                    let t = Self::entry_title(&entry);
                    if t < q_clean {
                        low = mid + 1;
                    } else {
                        high = mid;
                    }
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        // Scan matches from low
        let mut curr = low;
        while curr < count && results.len() < limit {
            if let Ok(Some(idx)) = listing.get(curr) {
                if let Ok(entry) = self.zim.get_by_url_index(idx) {
                    let t = Self::entry_title(&entry);
                    let t_lower = t.to_lowercase();

                    if t.starts_with(q_clean)
                        || t_lower.starts_with(&q_lower)
                        || entry.url.starts_with(q_clean)
                    {
                        results.push(ZimSuggestion {
                            title: t.to_string(),
                            path: entry.url.clone(),
                            book_id: self.info.id.clone(),
                        });
                    } else if !t.is_empty()
                        && !q_clean.starts_with(t)
                        && t > q_clean
                        && !t_lower.starts_with(&q_lower)
                    {
                        // Beyond prefix range
                        break;
                    }
                }
            }
            curr += 1;
        }

        Ok(results)
    }

    pub fn search(&self, query: &str, count: usize) -> Result<Vec<ZimSearchResult>, String> {
        let q_trim = query.trim();
        if q_trim.is_empty() {
            return Ok(Vec::new());
        }

        let mut results: Vec<ZimSearchResult> = Vec::new();
        let mut seen_urls = std::collections::HashSet::new();

        // 1. Direct suggestions / prefix matches
        if let Ok(suggs) = self.suggest(q_trim, count * 2) {
            for s in suggs {
                if seen_urls.insert(s.path.clone()) {
                    let snippet = self.extract_snippet(&s.path, q_trim);
                    results.push(ZimSearchResult {
                        title: s.title,
                        url: s.path,
                        snippet,
                        book_title: self.info.title.clone(),
                        book_id: self.info.id.clone(),
                    });
                    if results.len() >= count {
                        return Ok(results);
                    }
                }
            }
        }

        // 2. Token-based search if multi-term query
        let tokens: Vec<&str> = q_trim.split_whitespace().collect();
        if tokens.len() > 1 && results.len() < count {
            for token in &tokens {
                if token.len() < 2 {
                    continue;
                }
                if let Ok(suggs) = self.suggest(token, count * 2) {
                    for s in suggs {
                        // Check if title contains the other tokens
                        let t_lower = s.title.to_lowercase();
                        let all_match = tokens
                            .iter()
                            .all(|tok| t_lower.contains(&tok.to_lowercase()));
                        if all_match && seen_urls.insert(s.path.clone()) {
                            let snippet = self.extract_snippet(&s.path, q_trim);
                            results.push(ZimSearchResult {
                                title: s.title,
                                url: s.path,
                                snippet,
                                book_title: self.info.title.clone(),
                                book_id: self.info.id.clone(),
                            });
                            if results.len() >= count {
                                return Ok(results);
                            }
                        }
                    }
                }
            }
        }

        Ok(results)
    }

    fn extract_snippet(&self, path: &str, query: &str) -> String {
        if let Some(entry) = self.find_entry(path) {
            if let Ok(Some(content)) = self.zim.entry_content(&entry) {
                if let Ok(bytes) = content.to_vec() {
                    let html = String::from_utf8_lossy(&bytes);
                    let text = strip_html_tags(&html);

                    let q_lower = query.to_lowercase();
                    let t_lower = text.to_lowercase();

                    if let Some(pos) = t_lower.find(&q_lower) {
                        let start = pos.saturating_sub(60);
                        let end = (pos + query.len() + 140).min(text.len());
                        let slice = &text[start..end];
                        return format!("...{}...", slice.replace('\n', " ").trim());
                    }

                    // Otherwise return the first paragraph / lead text
                    let lead = text.chars().take(200).collect::<String>();
                    return format!("{}...", lead.replace('\n', " ").trim());
                }
            }
        }
        String::new()
    }
}

fn strip_html_tags(html: &str) -> String {
    let mut out = String::with_capacity(html.len() / 2);
    let mut in_tag = false;
    let mut in_script = false;
    let mut in_style = false;

    let mut i = 0;
    let chars: Vec<char> = html.chars().collect();
    while i < chars.len() {
        if chars[i] == '<' {
            in_tag = true;
            let rem: String = chars[i..].iter().take(10).collect();
            let rem_lower = rem.to_lowercase();
            if rem_lower.starts_with("<script") {
                in_script = true;
            } else if rem_lower.starts_with("<style") {
                in_style = true;
            } else if rem_lower.starts_with("</script") {
                in_script = false;
            } else if rem_lower.starts_with("</style") {
                in_style = false;
            }
        } else if chars[i] == '>' {
            in_tag = false;
            out.push(' ');
        } else if !in_tag && !in_script && !in_style {
            out.push(chars[i]);
        }
        i += 1;
    }

    decode_html_entities(out.trim())
}

fn url_decode(s: &str) -> String {
    let mut out = Vec::new();
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(val) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(val);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
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
