use async_trait::async_trait;
use chrono::Utc;

use crate::connector::{Connector, ConnectorError, FetchContext};
use crate::schema::document::Document;

pub struct NotionConnector {
    token: String,
    database_id: String,
}

impl NotionConnector {
    pub fn new(token: String, database_id: String) -> Self {
        Self { token, database_id }
    }
}

#[async_trait]
impl Connector for NotionConnector {
    fn source_type(&self) -> &str {
        "notion"
    }

    async fn fetch_incremental(&self, ctx: FetchContext) -> Result<Vec<Document>, ConnectorError> {
        let _ = ctx;
        if self.token.is_empty() {
            return Err(ConnectorError::Config(
                "Notion connector requires token".to_string(),
            ));
        }

        let url = format!(
            "https://api.notion.com/v1/databases/{}/query",
            self.database_id
        );

        let client = reqwest::Client::new();
        let response = client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Notion-Version", "2022-06-28")
            .json(&serde_json::json!({}))
            .send()
            .await
            .map_err(|e| ConnectorError::Http(e.to_string()))?;

        if !response.status().is_success() {
            return Err(ConnectorError::Http(format!(
                "Notion API returned status {}",
                response.status()
            )));
        }

        let body: serde_json::Value = response
            .json()
            .await
            .map_err(|e| ConnectorError::Parse(e.to_string()))?;

        let empty = vec![];
        let results = body["results"].as_array().unwrap_or(&empty);
        let mut docs = Vec::new();

        for page in results {
            let id = page["id"].as_str().unwrap_or("").to_string();
            let properties = &page["properties"];

            let title = properties
                .as_object()
                .and_then(|props| {
                    props.values().find(|v| v["type"] == "title")
                })
                .and_then(|t| t["title"].as_array())
                .and_then(|arr| arr.first())
                .and_then(|t| t["plain_text"].as_str())
                .unwrap_or("")
                .to_string();

            let content = page["properties"]
                .as_object()
                .and_then(|props| {
                    props.values().find(|v| v["type"] == "rich_text")
                })
                .and_then(|rt| rt["rich_text"].as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|t| t["plain_text"].as_str())
                        .collect::<Vec<_>>()
                        .join("")
                })
                .unwrap_or_default();

            let created_time = page["created_time"]
                .as_str()
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(Utc::now);
            let last_edited_time = page["last_edited_time"]
                .as_str()
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(Utc::now);

            let url = format!("https://notion.so/{}", id.replace('-', ""));

            docs.push(Document {
                source: "notion".to_string(),
                source_id: id,
                title,
                content,
                url: Some(url),
                author: None,
                created_at: created_time,
                updated_at: last_edited_time,
                tags: vec![],
                project: Some(self.database_id.clone()),
            });
        }

        Ok(docs)
    }
}
