use axum::extract::State;
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};

use crate::connector::{self, FetchContext};
use crate::engine::index_manager::IndexManager;
use crate::schema::document::Document;

#[derive(Debug, Deserialize)]
pub struct AddConnectorRequest {
    pub source_type: String,
    pub params: serde_json::Value,
    pub fetch_interval_secs: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct ConnectorInfo {
    pub source_type: String,
    pub params: serde_json::Value,
    pub status: String,
}

pub async fn add_connector(
    axum::Json(req): axum::Json<AddConnectorRequest>,
) -> impl IntoResponse {
    match connector::create_connector(&req.source_type, &req.params) {
        Ok(_) => axum::Json(serde_json::json!({
            "status": "ok",
            "message": format!("Connector {} added", req.source_type)
        }))
        .into_response(),
        Err(e) => (
            axum::http::StatusCode::BAD_REQUEST,
            axum::Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

pub async fn fetch_connector(
    State(manager): State<std::sync::Arc<IndexManager>>,
    axum::Json(req): axum::Json<FetchConnectorRequest>,
) -> impl IntoResponse {
    let conn = match connector::create_connector(&req.source_type, &req.params) {
        Ok(c) => c,
        Err(e) => {
            return (
                axum::http::StatusCode::BAD_REQUEST,
                axum::Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response()
        }
    };

    let ctx = FetchContext {
        last_fetched_at: req.last_fetched_at,
    };

    match conn.fetch_incremental(ctx).await {
        Ok(docs) => {
            let count = docs.len();
            match manager.add_documents(&docs).await {
                Ok(added) => {
                    if let Err(e) = manager.commit().await {
                        tracing::warn!("Post-fetch commit failed: {}", e);
                    }
                    axum::Json(serde_json::json!({
                        "status": "ok",
                        "fetched": count,
                        "indexed": added
                    }))
                    .into_response()
                }
                Err(e) => (
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    axum::Json(serde_json::json!({ "error": e })),
                )
                    .into_response(),
            }
        }
        Err(e) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            axum::Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

#[derive(Debug, Deserialize)]
pub struct FetchConnectorRequest {
    pub source_type: String,
    pub params: serde_json::Value,
    pub last_fetched_at: Option<chrono::DateTime<chrono::Utc>>,
}

pub async fn index_document(
    State(manager): State<std::sync::Arc<IndexManager>>,
    axum::Json(doc): axum::Json<Document>,
) -> impl IntoResponse {
    match manager.add_document(&doc).await {
        Ok(()) => axum::Json(serde_json::json!({
            "status": "ok",
            "message": "Document indexed"
        }))
        .into_response(),
        Err(e) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            axum::Json(serde_json::json!({ "error": e })),
        )
            .into_response(),
    }
}

pub async fn list_connectors() -> impl IntoResponse {
    let connectors = vec![
        ConnectorInfo {
            source_type: "local_markdown".to_string(),
            params: serde_json::json!({ "path": "./docs" }),
            status: "available".to_string(),
        },
        ConnectorInfo {
            source_type: "confluence".to_string(),
            params: serde_json::json!({ "base_url": "", "token": "", "space_key": "" }),
            status: "available".to_string(),
        },
        ConnectorInfo {
            source_type: "notion".to_string(),
            params: serde_json::json!({ "token": "", "database_id": "" }),
            status: "available".to_string(),
        },
        ConnectorInfo {
            source_type: "slack".to_string(),
            params: serde_json::json!({ "token": "", "channel": "" }),
            status: "available".to_string(),
        },
        ConnectorInfo {
            source_type: "maildir".to_string(),
            params: serde_json::json!({ "path": "" }),
            status: "available".to_string(),
        },
    ];
    axum::Json(serde_json::json!({ "connectors": connectors }))
}
