use json::JsonValue;

#[derive(Debug, Clone)]
pub struct KiwixBook {
    pub id: String,
    pub content_id: String,
    pub name: String,
    pub title: String,
    pub summary: String,
    pub language: String,
    pub article_count: u64,
    pub media_count: u64,
}

impl KiwixBook {
    pub fn to_json(&self) -> JsonValue {
        json::object! {
            "id" => self.id.clone(),
            "content_id" => self.content_id.clone(),
            "name" => self.name.clone(),
            "title" => self.title.clone(),
            "summary" => self.summary.clone(),
            "language" => self.language.clone(),
            "article_count" => self.article_count,
            "media_count" => self.media_count
        }
    }
}

#[derive(Debug, Clone)]
pub struct KiwixSearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
    pub book_title: String,
    pub details: String,
}

impl KiwixSearchResult {
    pub fn to_json(&self) -> JsonValue {
        json::object! {
            "title" => self.title.clone(),
            "url" => self.url.clone(),
            "snippet" => self.snippet.clone(),
            "book_title" => self.book_title.clone(),
            "details" => self.details.clone()
        }
    }
}
