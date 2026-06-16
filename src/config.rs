use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_index_dir")]
    pub index_dir: String,
    #[serde(default = "default_fetch_interval_secs")]
    pub fetch_interval_secs: u64,
    #[serde(default = "default_nrt_commit_interval_ms")]
    pub nrt_commit_interval_ms: u64,
    #[serde(default)]
    pub connectors: Vec<ConnectorConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectorConfig {
    pub source_type: String,
    #[serde(default)]
    pub params: serde_json::Value,
    #[serde(default = "default_fetch_interval_secs")]
    pub fetch_interval_secs: u64,
}

fn default_port() -> u16 {
    8340
}
fn default_index_dir() -> String {
    "./index_data".to_string()
}
fn default_fetch_interval_secs() -> u64 {
    300
}
fn default_nrt_commit_interval_ms() -> u64 {
    1000
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            port: default_port(),
            index_dir: default_index_dir(),
            fetch_interval_secs: default_fetch_interval_secs(),
            nrt_commit_interval_ms: default_nrt_commit_interval_ms(),
            connectors: vec![],
        }
    }
}
