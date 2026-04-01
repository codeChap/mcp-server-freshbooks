use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub client_id: String,
    pub client_secret: String,
    #[serde(default = "default_redirect_uri")]
    pub redirect_uri: String,
    pub account_id: Option<String>,
}

fn default_redirect_uri() -> String {
    "https://localhost/callback".into()
}

pub fn config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/root".into());
            PathBuf::from(home).join(".config")
        })
        .join("mcp-server-freshbooks")
}

pub fn config_path() -> PathBuf {
    config_dir().join("config.toml")
}

pub fn token_path() -> PathBuf {
    config_dir().join("token.json")
}

pub fn load() -> Result<Config> {
    let path = config_path();
    let content = std::fs::read_to_string(&path).with_context(|| {
        format!(
            "Failed to read config file: {}\n\
             Create it with your FreshBooks OAuth credentials.\n\
             Example:\n\n\
             client_id = \"your-client-id\"\n\
             client_secret = \"your-client-secret\"\n\
             redirect_uri = \"https://localhost/callback\"\n\
             account_id = \"your-account-id\"  # optional, auto-discovered",
            path.display()
        )
    })?;
    let config: Config =
        toml::from_str(&content).with_context(|| format!("Failed to parse {}", path.display()))?;

    if config.client_id.trim().is_empty() {
        bail!("client_id in {} is empty", path.display());
    }
    if config.client_secret.trim().is_empty() {
        bail!("client_secret in {} is empty", path.display());
    }

    Ok(config)
}
