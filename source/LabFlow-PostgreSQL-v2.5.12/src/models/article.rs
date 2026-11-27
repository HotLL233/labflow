use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Clone)]
pub struct HelpArticle {
    pub id: i64,
    pub title: String,
    pub content_html: String,
    pub toc_json: Option<String>,
    pub source_file: Option<String>,
    pub is_visible: bool,
    pub sort_order: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct HelpArticleUpdate {
    pub title: Option<String>,
    pub content_html: Option<String>,
    pub toc_json: Option<String>,
    pub is_visible: Option<bool>,
    pub sort_order: Option<i64>,
}
