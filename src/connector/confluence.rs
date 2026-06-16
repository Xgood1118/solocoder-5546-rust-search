use async_trait::async_trait;
use chrono::Utc;

use crate::connector::{Connector, ConnectorError, FetchContext};
use crate::schema::document::Document;

pub struct ConfluenceConnector {
    base_url: String,
    token: String,
    space_key: String,
}

impl ConfluenceConnector {
    pub fn new(base_url: String, token: String, space_key: String) -> Self {
        Self { base_url, token, space_key }
    }
}

#[async_trait]
impl Connector for ConfluenceConnector {
    fn source_type(&self) -> &str {
        "confluence"
    }

    async fn fetch_incremental(&self, ctx: FetchContext) -> Result<Vec<Document>, ConnectorError> {
        let _ = ctx;
        if self.base_url.is_empty() || self.token.is_empty() {
            return Err(ConnectorError::Config(
                "Confluence connector requires base_url and token".to_string(),
            ));
        }

        let url = format!(
            "{}/rest/api/content?spaceKey={}&limit=50&expand=body.storage,version",
            self.base_url.trim_end_matches('/'),
            self.space_key
        );

        let client = reqwest_compat();
        let response = client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .send()
            .await
            .map_err(|e| ConnectorError::Http(e.to_string()))?;

        if !response.status().is_success() {
            return Err(ConnectorError::Http(format!(
                "Confluence API returned status {}",
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
            let title = page["title"].as_str().unwrap_or("").to_string();
            let content = page["body"]["storage"]["value"]
                .as_str()
                .unwrap_or("")
                .to_string();
            let version = page["version"]["number"].as_i64().unwrap_or(0);
            let by = page["version"]["by"]["displayName"]
                .as_str()
                .unwrap_or("")
                .to_string();
            let when_str = page["version"]["when"]
                .as_str()
                .unwrap_or("");
            let updated_at = chrono::DateTime::parse_from_rfc3339(when_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());

            docs.push(Document {
                source: "confluence".to_string(),
                source_id: id.clone(),
                title,
                content,
                url: Some(format!("{}/pages/viewpage.action?pageId={}", self.base_url.trim_end_matches('/'), id)),
                author: Some(by),
                created_at: updated_at,
                updated_at,
                tags: vec![format!("v{}", version)],
                project: Some(self.space_key.clone()),
            });
        }

        Ok(docs)
    }
}

fn reqwest_compat() -> reqwest::Client {
    reqwest::Client::new()
}
