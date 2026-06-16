pub mod local_markdown;
pub mod confluence;
pub mod notion;
pub mod slack;
pub mod maildir;

use async_trait::async_trait;
use crate::schema::document::Document;

#[derive(Debug, Clone)]
pub struct FetchContext {
    pub last_fetched_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[async_trait]
pub trait Connector: Send + Sync {
    fn source_type(&self) -> &str;

    async fn fetch_incremental(&self, ctx: FetchContext) -> Result<Vec<Document>, ConnectorError>;

    async fn fetch_all(&self) -> Result<Vec<Document>, ConnectorError> {
        self.fetch_incremental(FetchContext { last_fetched_at: None }).await
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConnectorError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("HTTP error: {0}")]
    Http(String),
    #[error("Parse error: {0}")]
    Parse(String),
    #[error("Config error: {0}")]
    Config(String),
    #[error("{0}")]
    Other(String),
}

pub fn create_connector(source_type: &str, params: &serde_json::Value) -> Result<Box<dyn Connector>, ConnectorError> {
    match source_type {
        "local_markdown" => {
            let dir = params["path"].as_str().unwrap_or("./docs").to_string();
            Ok(Box::new(local_markdown::LocalMarkdownConnector::new(dir)))
        }
        "confluence" => {
            let base_url = params["base_url"].as_str().unwrap_or("").to_string();
            let token = params["token"].as_str().unwrap_or("").to_string();
            let space_key = params["space_key"].as_str().unwrap_or("").to_string();
            Ok(Box::new(confluence::ConfluenceConnector::new(base_url, token, space_key)))
        }
        "notion" => {
            let token = params["token"].as_str().unwrap_or("").to_string();
            let database_id = params["database_id"].as_str().unwrap_or("").to_string();
            Ok(Box::new(notion::NotionConnector::new(token, database_id)))
        }
        "slack" => {
            let token = params["token"].as_str().unwrap_or("").to_string();
            let channel = params["channel"].as_str().unwrap_or("").to_string();
            Ok(Box::new(slack::SlackConnector::new(token, channel)))
        }
        "maildir" => {
            let path = params["path"].as_str().unwrap_or("").to_string();
            Ok(Box::new(maildir::MaildirConnector::new(path)))
        }
        _ => Err(ConnectorError::Config(format!("Unknown source type: {}", source_type))),
    }
}
