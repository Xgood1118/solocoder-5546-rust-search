use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub source: String,
    pub source_id: String,
    pub title: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
}

impl Document {
    pub fn unique_key(&self) -> String {
        format!("{}:{}", self.source, self.source_id)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHit {
    pub doc: Document,
    pub score: f32,
    pub snippet: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FacetBucket {
    pub value: String,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Facets {
    pub source: Vec<FacetBucket>,
    pub author: Vec<FacetBucket>,
    pub tags: Vec<FacetBucket>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResponse {
    pub total: usize,
    pub hits: Vec<SearchHit>,
    pub facets: Facets,
    pub query: String,
    pub elapsed_ms: u64,
}
