use axum::extract::{Query, State};
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};

use crate::engine::index_manager::IndexManager;

#[derive(Debug, Deserialize)]
pub struct SearchParams {
    pub q: String,
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default = "default_offset")]
    pub offset: usize,
    pub source: Option<String>,
    pub author: Option<String>,
    pub tags: Option<String>,
    pub from_date: Option<String>,
    pub to_date: Option<String>,
}

fn default_limit() -> usize { 20 }
fn default_offset() -> usize { 0 }

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SearchFilters {
    pub source: Option<Vec<String>>,
    pub author: Option<Vec<String>>,
    pub tags: Option<Vec<String>>,
    pub from_date: Option<i64>,
    pub to_date: Option<i64>,
}

impl From<&SearchParams> for SearchFilters {
    fn from(params: &SearchParams) -> Self {
        Self {
            source: params.source.as_ref().map(|s| s.split(',').map(|v| v.trim().to_string()).collect()),
            author: params.author.as_ref().map(|s| s.split(',').map(|v| v.trim().to_string()).collect()),
            tags: params.tags.as_ref().map(|s| s.split(',').map(|v| v.trim().to_string()).collect()),
            from_date: params.from_date.as_ref().and_then(|d| chrono::DateTime::parse_from_rfc3339(&format!("{}T00:00:00Z", d)).ok().map(|dt| dt.timestamp())),
            to_date: params.to_date.as_ref().and_then(|d| chrono::DateTime::parse_from_rfc3339(&format!("{}T23:59:59Z", d)).ok().map(|dt| dt.timestamp())),
        }
    }
}

#[derive(Deserialize, Clone)]
pub struct SearchRequestBody {
    pub q: String,
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default = "default_offset")]
    pub offset: usize,
    pub source: Option<Vec<String>>,
    pub author: Option<Vec<String>>,
    pub tags: Option<Vec<String>>,
    pub from_date: Option<String>,
    pub to_date: Option<String>,
}

impl From<&SearchRequestBody> for SearchFilters {
    fn from(body: &SearchRequestBody) -> Self {
        Self {
            source: body.source.clone(),
            author: body.author.clone(),
            tags: body.tags.clone(),
            from_date: body.from_date.as_ref().and_then(|d| chrono::DateTime::parse_from_rfc3339(&format!("{}T00:00:00Z", d)).ok().map(|dt| dt.timestamp())),
            to_date: body.to_date.as_ref().and_then(|d| chrono::DateTime::parse_from_rfc3339(&format!("{}T23:59:59Z", d)).ok().map(|dt| dt.timestamp())),
        }
    }
}

pub async fn search_get(
    State(manager): State<std::sync::Arc<IndexManager>>,
    Query(params): Query<SearchParams>,
) -> impl IntoResponse {
    let filters = SearchFilters::from(&params);
    match manager.search(&params.q, &filters, params.limit, params.offset) {
        Ok(resp) => axum::Json(resp).into_response(),
        Err(e) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            axum::Json(serde_json::json!({ "error": e })),
        )
            .into_response(),
    }
}

pub async fn search_post(
    State(manager): State<std::sync::Arc<IndexManager>>,
    axum::Json(body): axum::Json<SearchRequestBody>,
) -> impl IntoResponse {
    let filters = SearchFilters::from(&body);
    match manager.search(&body.q, &filters, body.limit, body.offset) {
        Ok(resp) => axum::Json(resp).into_response(),
        Err(e) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            axum::Json(serde_json::json!({ "error": e })),
        )
            .into_response(),
    }
}
