use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::path::PathBuf;

use crate::connector::{Connector, ConnectorError, FetchContext};
use crate::schema::document::Document;

pub struct MaildirConnector {
    path: String,
}

impl MaildirConnector {
    pub fn new(path: String) -> Self {
        Self { path }
    }
}

#[async_trait]
impl Connector for MaildirConnector {
    fn source_type(&self) -> &str {
        "maildir"
    }

    async fn fetch_incremental(&self, ctx: FetchContext) -> Result<Vec<Document>, ConnectorError> {
        let root = PathBuf::from(&self.path);
        if !root.exists() {
            return Err(ConnectorError::Config(format!(
                "Maildir path does not exist: {}",
                self.path
            )));
        }

        let mut docs = Vec::new();
        let cur_dir = root.join("cur");
        let new_dir = root.join("new");

        for mail_dir in &[cur_dir, new_dir] {
            if !mail_dir.exists() {
                continue;
            }
            let mut entries = tokio::fs::read_dir(mail_dir).await?;
            while let Some(entry) = entries.next_entry().await? {
                let path = entry.path();
                if !path.is_file() {
                    continue;
                }

                if let Some(last_fetched) = ctx.last_fetched_at {
                    if let Ok(metadata) = entry.metadata().await {
                        if let Ok(modified) = metadata.modified() {
                            let modified_dt: DateTime<Utc> = modified.into();
                            if modified_dt < last_fetched {
                                continue;
                            }
                        }
                    }
                }

                let content = match tokio::fs::read_to_string(&path).await {
                    Ok(c) => c,
                    Err(e) => {
                        tracing::warn!("Failed to read mail {:?}: {}", path, e);
                        continue;
                    }
                };

                let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("unknown");
                let (subject, from, date, body) = parse_simple_email(&content);

                let created_at = date.unwrap_or_else(Utc::now);

                docs.push(Document {
                    source: "maildir".to_string(),
                    source_id: filename.to_string(),
                    title: subject,
                    content: body,
                    url: Some(format!("maildir://{}", filename)),
                    author: from,
                    created_at,
                    updated_at: created_at,
                    tags: vec!["email".to_string()],
                    project: None,
                });
            }
        }

        Ok(docs)
    }
}

fn parse_simple_email(raw: &str) -> (String, Option<String>, Option<DateTime<Utc>>, String) {
    let mut subject = String::from("(no subject)");
    let mut from = None;
    let mut date = None;
    let mut body_start = 0;

    for (i, line) in raw.lines().enumerate() {
        if line.is_empty() {
            body_start = i + 1;
            break;
        }
        let lower = line.to_lowercase();
        if lower.starts_with("subject:") {
            subject = line[8..].trim().to_string();
        } else if lower.starts_with("from:") {
            from = Some(line[5..].trim().to_string());
        } else if lower.starts_with("date:") {
            let date_str = line[5..].trim();
            date = chrono::DateTime::parse_from_rfc2822(date_str)
                .map(|d| d.with_timezone(&Utc))
                .ok();
        }
    }

    let body = raw.lines().skip(body_start).collect::<Vec<_>>().join("\n");
    (subject, from, date, body)
}
