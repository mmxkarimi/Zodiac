use json::JsonValue;

#[derive(Debug, Clone)]
pub struct ZimBookInfo {
    pub id: String,
    pub name: String,
    pub title: String,
    pub summary: String,
    pub language: String,
    pub article_count: u64,
    pub file_path: String,
    pub file_size_bytes: u64,
}

impl ZimBookInfo {
    pub fn to_json(&self) -> JsonValue {
        json::object! {
            "id" => self.id.clone(),
            "name" => self.name.clone(),
            "title" => self.title.clone(),
            "summary" => self.summary.clone(),
            "language" => self.language.clone(),
            "article_count" => self.article_count,
            "file_path" => self.file_path.clone(),
            "file_size_bytes" => self.file_size_bytes
        }
    }
}

#[derive(Debug, Clone)]
pub struct ZimSearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
    pub book_title: String,
    pub book_id: String,
}

impl ZimSearchResult {
    pub fn to_json(&self) -> JsonValue {
        json::object! {
            "title" => self.title.clone(),
            "url" => self.url.clone(),
            "snippet" => self.snippet.clone(),
            "book_title" => self.book_title.clone(),
            "book_id" => self.book_id.clone()
        }
    }
}

#[derive(Debug, Clone)]
pub struct ZimSuggestion {
    pub title: String,
    pub path: String,
    pub book_id: String,
}

impl ZimSuggestion {
    pub fn to_json(&self) -> JsonValue {
        json::object! {
            "title" => self.title.clone(),
            "path" => self.path.clone(),
            "book_id" => self.book_id.clone()
        }
    }
}
