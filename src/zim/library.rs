use super::archive::ZimArchive;
use super::models::{ZimBookInfo, ZimSearchResult};
use json::JsonValue;
use std::path::Path;
use std::time::Instant;

pub struct ZimLibrary {
    pub archives: Vec<ZimArchive>,
}

impl ZimLibrary {
    pub fn new() -> Self {
        Self {
            archives: Vec::new(),
        }
    }

    pub fn load_file<P: AsRef<Path>>(&mut self, path: P) -> Result<(), String> {
        let archive = ZimArchive::open(path)?;
        self.archives.push(archive);
        Ok(())
    }

    pub fn is_empty(&self) -> bool {
        self.archives.is_empty()
    }

    pub fn len(&self) -> usize {
        self.archives.len()
    }

    pub fn list_books(&self) -> Vec<ZimBookInfo> {
        self.archives.iter().map(|a| a.info.clone()).collect()
    }

    pub fn resolve_archive(&self, book: Option<&str>) -> Result<&ZimArchive, String> {
        if self.archives.is_empty() {
            return Err("No ZIM archives loaded. Start Zodiac with --zim <file.zim>".to_string());
        }

        if let Some(b) = book {
            let b_trimmed = b.trim();
            if let Some(found) = self.archives.iter().find(|a| a.info.id == b_trimmed) {
                return Ok(found);
            }
            if let Some(found) = self.archives.iter().find(|a| a.info.name == b_trimmed) {
                return Ok(found);
            }
            if let Some(found) = self.archives.iter().find(|a| a.info.title == b_trimmed) {
                return Ok(found);
            }
            if let Some(found) = self
                .archives
                .iter()
                .find(|a| a.info.id.starts_with(b_trimmed))
            {
                return Ok(found);
            }
            if let Some(found) = self
                .archives
                .iter()
                .find(|a| a.info.name.starts_with(b_trimmed))
            {
                return Ok(found);
            }
            Err(format!(
                "Book '{}' not found among loaded archives. Available: {:?}",
                b_trimmed,
                self.archives.iter().map(|a| &a.info.id).collect::<Vec<_>>()
            ))
        } else {
            Ok(&self.archives[0])
        }
    }

    pub fn suggest(
        &self,
        query: &str,
        book: Option<&str>,
        limit: Option<usize>,
    ) -> Result<JsonValue, String> {
        let limit = limit.unwrap_or(10);
        let archive = self.resolve_archive(book)?;
        let suggestions = archive.suggest(query, limit)?;

        let mut arr = JsonValue::new_array();
        for s in suggestions {
            let _ = arr.push(s.to_json());
        }
        Ok(arr)
    }

    pub fn search(
        &self,
        query: &str,
        book: Option<&str>,
        count: Option<usize>,
    ) -> Result<Vec<ZimSearchResult>, String> {
        let count = count.unwrap_or(10);

        if let Some(b) = book {
            let archive = self.resolve_archive(Some(b))?;
            archive.search(query, count)
        } else if self.archives.len() == 1 {
            self.archives[0].search(query, count)
        } else {
            // Search across all archives and aggregate
            let mut all_results = Vec::new();
            for archive in &self.archives {
                if let Ok(results) = archive.search(query, count) {
                    all_results.extend(results);
                    if all_results.len() >= count {
                        break;
                    }
                }
            }
            all_results.truncate(count);
            Ok(all_results)
        }
    }

    pub fn read_page(
        &self,
        book: &str,
        path: &str,
        raw_html: bool,
        max_length: Option<usize>,
    ) -> Result<String, String> {
        let archive = self.resolve_archive(Some(book))?;
        archive.read_page(path, raw_html, max_length)
    }

    pub fn random_page(&self, book: Option<&str>) -> Result<JsonValue, String> {
        let archive = self.resolve_archive(book)?;
        archive.random_page()
    }

    pub fn server_status(&self) -> Result<JsonValue, String> {
        let start = Instant::now();
        let latency_us = start.elapsed().as_micros() as u64;

        let mut book_summaries = JsonValue::new_array();
        let mut total_articles: u64 = 0;

        for a in &self.archives {
            total_articles += a.info.article_count;
            let _ = book_summaries.push(json::object! {
                "id" => a.info.id.clone(),
                "title" => a.info.title.clone(),
                "language" => a.info.language.clone(),
                "article_count" => a.info.article_count,
                "file_path" => a.info.file_path.clone(),
                "file_size_bytes" => a.info.file_size_bytes
            });
        }

        Ok(json::object! {
            "status" => "online",
            "backend" => "embedded_zim_reader",
            "latency_us" => latency_us,
            "books_count" => self.archives.len(),
            "total_articles" => total_articles,
            "books" => book_summaries
        })
    }
}
