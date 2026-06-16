use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::path::PathBuf;
use walkdir::WalkDir;

use crate::connector::{Connector, ConnectorError, FetchContext};
use crate::schema::document::Document;

pub struct LocalMarkdownConnector {
    root_dir: String,
}

impl LocalMarkdownConnector {
    pub fn new(root_dir: String) -> Self {
        Self { root_dir }
    }
}

#[async_trait]
impl Connector for LocalMarkdownConnector {
    fn source_type(&self) -> &str {
        "local_markdown"
    }

    async fn fetch_incremental(&self, ctx: FetchContext) -> Result<Vec<Document>, ConnectorError> {
        let root = PathBuf::from(&self.root_dir);
        if !root.exists() {
            return Err(ConnectorError::Config(format!(
                "Directory does not exist: {}",
                self.root_dir
            )));
        }

        let mut docs = Vec::new();

        for entry in WalkDir::new(&root).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if ext != "md" && ext != "markdown" {
                continue;
            }

            if let Some(last_fetched) = ctx.last_fetched_at {
                if let Ok(metadata) = entry.metadata() {
                    if let Ok(modified) = metadata.modified() {
                        let modified_dt: DateTime<Utc> = modified.into();
                        if modified_dt < last_fetched {
                            continue;
                        }
                    }
                }
            }

            match tokio::fs::read_to_string(path).await {
                Ok(content) => {
                    let relative = path.strip_prefix(&root).unwrap_or(path);
                    let source_id = relative.to_string_lossy().replace('\\', "/");
                    let title = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("untitled")
                        .to_string();

                    let metadata = entry.metadata().ok();
                    let created_at = metadata
                        .as_ref()
                        .and_then(|m| m.created().ok())
                        .map(|t| -> DateTime<Utc> { t.into() })
                        .unwrap_or_else(Utc::now);
                    let updated_at = metadata
                        .as_ref()
                        .and_then(|m| m.modified().ok())
                        .map(|t| -> DateTime<Utc> { t.into() })
                        .unwrap_or_else(Utc::now);

                    docs.push(Document {
                        source: "local_markdown".to_string(),
                        source_id,
                        title,
                        content,
                        url: Some(format!("file:///{}", relative.to_string_lossy().replace('\\', "/"))),
                        author: None,
                        created_at,
                        updated_at,
                        tags: vec![],
                        project: None,
                    });
                }
                Err(e) => {
                    tracing::warn!("Failed to read {:?}: {}", path, e);
                }
            }
        }

        Ok(docs)
    }
}
