use async_trait::async_trait;
use chrono::Utc;

use crate::connector::{Connector, ConnectorError, FetchContext};
use crate::schema::document::Document;

pub struct SlackConnector {
    token: String,
    channel: String,
}

impl SlackConnector {
    pub fn new(token: String, channel: String) -> Self {
        Self { token, channel }
    }
}

#[async_trait]
impl Connector for SlackConnector {
    fn source_type(&self) -> &str {
        "slack"
    }

    async fn fetch_incremental(&self, ctx: FetchContext) -> Result<Vec<Document>, ConnectorError> {
        if self.token.is_empty() {
            return Err(ConnectorError::Config(
                "Slack connector requires token".to_string(),
            ));
        }

        let url = "https://slack.com/api/conversations.history".to_string();
        let client = reqwest::Client::new();

        let mut params = vec![("channel", self.channel.as_str()), ("limit", "200")];

        let oldest = ctx.last_fetched_at.map(|t| t.timestamp().to_string());
        let oldest_str;
        if let Some(ref ts) = oldest {
            oldest_str = ts.clone();
            params.push(("oldest", oldest_str.as_str()));
        }

        let response = client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .query(&params)
            .send()
            .await
            .map_err(|e| ConnectorError::Http(e.to_string()))?;

        if !response.status().is_success() {
            return Err(ConnectorError::Http(format!(
                "Slack API returned status {}",
                response.status()
            )));
        }

        let body: serde_json::Value = response
            .json()
            .await
            .map_err(|e| ConnectorError::Parse(e.to_string()))?;

        if body["ok"].as_bool() != Some(true) {
            return Err(ConnectorError::Http(format!(
                "Slack API error: {}",
                body["error"].as_str().unwrap_or("unknown")
            )));
        }

        let empty = vec![];
        let messages = body["messages"].as_array().unwrap_or(&empty);
        let mut docs = Vec::new();

        for msg in messages {
            let ts = msg["ts"].as_str().unwrap_or("").to_string();
            let text = msg["text"].as_str().unwrap_or("").to_string();
            let user = msg["user"].as_str().unwrap_or("unknown").to_string();
            let thread_ts = msg["thread_ts"].as_str().unwrap_or(&ts);

            let created_at = ts
                .parse::<f64>()
                .ok()
                .and_then(|t| chrono::DateTime::from_timestamp(t as i64, 0))
                .unwrap_or_else(Utc::now);

            docs.push(Document {
                source: "slack".to_string(),
                source_id: ts.clone(),
                title: format!("Slack message {}", thread_ts),
                content: text,
                url: Some(format!(
                    "https://slack.com/archives/{}/p{}",
                    self.channel,
                    ts.replace('.', "")
                )),
                author: Some(user),
                created_at,
                updated_at: created_at,
                tags: vec!["slack".to_string()],
                project: Some(self.channel.clone()),
            });
        }

        Ok(docs)
    }
}
