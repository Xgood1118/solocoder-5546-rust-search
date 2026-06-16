use std::collections::HashMap;
use std::sync::Arc;

use chrono::Utc;
use tokio::sync::RwLock;

use crate::connector::{self, FetchContext};
use crate::engine::index_manager::IndexManager;

#[derive(Clone)]
struct ConnectorState {
    source_type: String,
    params: serde_json::Value,
    fetch_interval_secs: u64,
    last_fetched_at: Option<chrono::DateTime<chrono::Utc>>,
}

pub struct Scheduler {
    connectors: Arc<RwLock<HashMap<String, ConnectorState>>>,
    index_manager: Arc<IndexManager>,
}

impl Scheduler {
    pub fn new(index_manager: Arc<IndexManager>) -> Self {
        Self {
            connectors: Arc::new(RwLock::new(HashMap::new())),
            index_manager,
        }
    }

    pub async fn add_connector(
        &self,
        source_type: String,
        params: serde_json::Value,
        fetch_interval_secs: u64,
    ) -> Result<(), String> {
        let key = source_type.clone();
        let state = ConnectorState {
            source_type,
            params,
            fetch_interval_secs,
            last_fetched_at: None,
        };
        self.connectors.write().await.insert(key, state);
        Ok(())
    }

    pub async fn start(self: Arc<Self>) {
        let this = self.clone();
        tokio::spawn(async move {
            loop {
                this.run_cycle().await;
                tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
            }
        });
    }

    async fn run_cycle(&self) {
        let connectors = self.connectors.read().await;
        for (key, state) in connectors.iter() {
            let should_fetch = match state.last_fetched_at {
                Some(last) => {
                    let elapsed = Utc::now()
                        .signed_duration_since(last)
                        .num_seconds();
                    elapsed as u64 >= state.fetch_interval_secs
                }
                None => true,
            };

            if !should_fetch {
                continue;
            }

            let conn = match connector::create_connector(&state.source_type, &state.params) {
                Ok(c) => c,
                Err(e) => {
                    tracing::error!("Connector {} creation failed: {}", key, e);
                    continue;
                }
            };

            let ctx = FetchContext {
                last_fetched_at: state.last_fetched_at,
            };

            match conn.fetch_incremental(ctx).await {
                Ok(docs) => {
                    let count = docs.len();
                    if count > 0 {
                        tracing::info!("Connector {} fetched {} documents", key, count);
                        match self.index_manager.add_documents(&docs).await {
                            Ok(added) => {
                                tracing::info!("Connector {} indexed {} documents", key, added);
                            }
                            Err(e) => {
                                tracing::error!("Connector {} index failed: {}", key, e);
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("Connector {} fetch failed: {}", key, e);
                }
            }

            drop(conn);
        }

        if let Err(e) = self.index_manager.commit_if_dirty().await {
            tracing::error!("Scheduler commit failed: {}", e);
        }
    }

    pub async fn start_nrt_commit(index_manager: Arc<IndexManager>, interval_ms: u64) {
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(tokio::time::Duration::from_millis(interval_ms)).await;
                if let Err(e) = index_manager.commit_if_dirty().await {
                    tracing::error!("NRT commit failed: {}", e);
                }
            }
        });
    }
}
