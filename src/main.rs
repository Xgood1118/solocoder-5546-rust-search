use std::sync::Arc;

use axum::routing::{get, post};
use axum::Router;
use tower_http::cors::CorsLayer;

use unified_search::api::{
    commit::force_commit,
    connector_api::{add_connector, fetch_connector, index_document, list_connectors},
    search::{search_get, search_post},
};
use unified_search::config::AppConfig;
use unified_search::engine::index_manager::IndexManager;
use unified_search::scheduler::Scheduler;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "unified_search=info,tantivy=warn".into()),
        )
        .init();

    let config: AppConfig = match std::fs::read_to_string("config.json") {
        Ok(content) => serde_json::from_str(&content).unwrap_or_else(|e| {
            tracing::warn!("Failed to parse config.json: {}, using defaults", e);
            AppConfig::default()
        }),
        Err(_) => {
            tracing::info!("No config.json found, using defaults");
            AppConfig::default()
        }
    };

    let port = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(config.port);

    let index_manager = Arc::new(
        IndexManager::open(&config.index_dir).expect("Failed to open index"),
    );

    let scheduler = Arc::new(Scheduler::new(index_manager.clone()));

    Scheduler::start_nrt_commit(index_manager.clone(), config.nrt_commit_interval_ms).await;

    for connector_cfg in &config.connectors {
        if let Err(e) = scheduler
            .add_connector(
                connector_cfg.source_type.clone(),
                connector_cfg.params.clone(),
                connector_cfg.fetch_interval_secs,
            )
            .await
        {
            tracing::error!("Failed to add connector {}: {}", connector_cfg.source_type, e);
        }
    }

    let scheduler_clone = scheduler.clone();
    tokio::spawn(async move {
        scheduler_clone.start().await;
    });

    let app = Router::new()
        .route("/search", get(search_get).post(search_post))
        .route("/commit", post(force_commit))
        .route("/connectors", get(list_connectors).post(add_connector))
        .route("/connectors/fetch", post(fetch_connector))
        .route("/documents", post(index_document))
        .layer(CorsLayer::permissive())
        .with_state(index_manager);

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("Unified Search API listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
