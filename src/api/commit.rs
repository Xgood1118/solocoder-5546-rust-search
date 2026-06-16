use axum::extract::State;
use axum::response::IntoResponse;

use crate::engine::index_manager::IndexManager;

pub async fn force_commit(
    State(manager): State<std::sync::Arc<IndexManager>>,
) -> impl IntoResponse {
    match manager.force_commit().await {
        Ok(()) => axum::Json(serde_json::json!({
            "status": "ok",
            "message": "Index committed successfully"
        }))
        .into_response(),
        Err(e) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            axum::Json(serde_json::json!({ "error": e })),
        )
            .into_response(),
    }
}
