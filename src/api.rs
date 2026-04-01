use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;
use std::time::Duration;
use thiserror::Error;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::config::Config;

const FB_API_BASE: &str = "https://api.freshbooks.com";
const FB_AUTH_URL: &str = "https://auth.freshbooks.com/oauth/authorize";

/// Token lifetime before we refresh (11 hours, tokens last 12).
const REFRESH_AFTER_SECS: f64 = 39600.0;

#[derive(Error, Debug)]
pub enum ApiError {
    #[error("HTTP request failed: {0}")]
    Reqwest(#[from] reqwest::Error),

    #[error("FreshBooks API error ({status}): {body}")]
    Api {
        status: reqwest::StatusCode,
        body: String,
    },

    #[error("Not authenticated. Use get_auth_url and exchange_code tools first.")]
    NoToken,

    #[error("{0}")]
    Other(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenData {
    pub access_token: String,
    pub refresh_token: String,
    #[serde(default)]
    pub expires_in: Option<u64>,
    #[serde(default)]
    pub scope: Option<String>,
    #[serde(default)]
    pub saved_at: Option<f64>,
    /// Preserve any extra fields from FreshBooks.
    #[serde(flatten)]
    pub extra: serde_json::Map<String, Value>,
}

pub struct FreshBooksClient {
    client_id: String,
    client_secret: String,
    redirect_uri: String,
    account_id: RwLock<Option<String>>,
    token: RwLock<Option<TokenData>>,
    token_path: PathBuf,
    http: Client,
}

impl FreshBooksClient {
    pub fn new(cfg: Config) -> Self {
        let token_path = crate::config::token_path();

        // Try to load existing token from disk.
        let token = std::fs::read_to_string(&token_path)
            .ok()
            .and_then(|s| serde_json::from_str::<TokenData>(&s).ok());

        if token.is_some() {
            info!("loaded existing token from {}", token_path.display());
        }

        Self {
            client_id: cfg.client_id,
            client_secret: cfg.client_secret,
            redirect_uri: cfg.redirect_uri,
            account_id: RwLock::new(cfg.account_id),
            token: RwLock::new(token),
            token_path,
            http: Client::builder()
                .timeout(Duration::from_secs(60))
                .build()
                .expect("Failed to build reqwest client"),
        }
    }

    /// Generate the OAuth2 authorization URL.
    pub fn auth_url(&self) -> String {
        format!(
            "{FB_AUTH_URL}?response_type=code&client_id={}&redirect_uri={}",
            self.client_id, self.redirect_uri,
        )
    }

    /// Exchange an authorization code for tokens.
    pub async fn exchange_code(&self, code: &str) -> Result<TokenData, ApiError> {
        let body = serde_json::json!({
            "grant_type": "authorization_code",
            "client_id": self.client_id,
            "client_secret": self.client_secret,
            "code": code,
            "redirect_uri": self.redirect_uri,
        });

        let resp = self
            .http
            .post(format!("{FB_API_BASE}/auth/oauth/token"))
            .json(&body)
            .send()
            .await?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(ApiError::Api { status, body });
        }

        let mut token: TokenData = resp.json().await?;
        token.saved_at = Some(now_epoch());
        self.save_token(&token)?;

        *self.token.write().await = Some(token.clone());
        Ok(token)
    }

    /// Ensure we have a valid (non-expired) access token. Refreshes if needed.
    async fn ensure_token(&self) -> Result<String, ApiError> {
        // Check if we need to refresh.
        {
            let token = self.token.read().await;
            if let Some(ref t) = *token {
                let elapsed = now_epoch() - t.saved_at.unwrap_or(0.0);
                if elapsed < REFRESH_AFTER_SECS {
                    return Ok(t.access_token.clone());
                }
                debug!("token expired ({elapsed:.0}s old), refreshing");
            } else {
                return Err(ApiError::NoToken);
            }
        }

        // Refresh.
        let refresh_tok = {
            let token = self.token.read().await;
            token
                .as_ref()
                .map(|t| t.refresh_token.clone())
                .ok_or(ApiError::NoToken)?
        };

        let body = serde_json::json!({
            "grant_type": "refresh_token",
            "client_id": self.client_id,
            "client_secret": self.client_secret,
            "refresh_token": refresh_tok,
            "redirect_uri": self.redirect_uri,
        });

        let resp = self
            .http
            .post(format!("{FB_API_BASE}/auth/oauth/token"))
            .json(&body)
            .send()
            .await?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            warn!("token refresh failed: {body}");
            return Err(ApiError::Api { status, body });
        }

        let mut token: TokenData = resp.json().await?;
        token.saved_at = Some(now_epoch());
        self.save_token(&token)?;
        let access = token.access_token.clone();
        *self.token.write().await = Some(token);
        info!("token refreshed successfully");
        Ok(access)
    }

    /// Get the account_id, auto-discovering if not set.
    pub async fn account_id(&self) -> Result<String, ApiError> {
        {
            let id = self.account_id.read().await;
            if let Some(ref aid) = *id {
                return Ok(aid.clone());
            }
        }

        // Auto-discover from /users/me.
        info!("auto-discovering account_id from /auth/api/v1/users/me");
        let data = self.get("/auth/api/v1/users/me").await?;

        let account_id = data["response"]["business_memberships"]
            .as_array()
            .and_then(|memberships| {
                memberships.iter().find_map(|m| {
                    m["business"]["account_id"].as_str().map(|s| s.to_string())
                })
            })
            .ok_or_else(|| {
                ApiError::Other("Could not find account_id in /users/me response".into())
            })?;

        *self.account_id.write().await = Some(account_id.clone());
        Ok(account_id)
    }

    /// GET request to FreshBooks API.
    pub async fn get(&self, path: &str) -> Result<Value, ApiError> {
        let token = self.ensure_token().await?;
        let url = format!("{FB_API_BASE}{path}");
        debug!(url = %url, "GET");

        let resp = self
            .http
            .get(&url)
            .header("Authorization", format!("Bearer {token}"))
            .header("Content-Type", "application/json")
            .header("Api-Version", "alpha")
            .send()
            .await?;

        self.handle_response(resp).await
    }

    /// POST request to FreshBooks API.
    pub async fn post(&self, path: &str, body: &Value) -> Result<Value, ApiError> {
        let token = self.ensure_token().await?;
        let url = format!("{FB_API_BASE}{path}");
        debug!(url = %url, "POST");

        let resp = self
            .http
            .post(&url)
            .header("Authorization", format!("Bearer {token}"))
            .header("Content-Type", "application/json")
            .header("Api-Version", "alpha")
            .json(body)
            .send()
            .await?;

        self.handle_response(resp).await
    }

    /// PUT request to FreshBooks API.
    pub async fn put(&self, path: &str, body: &Value) -> Result<Value, ApiError> {
        let token = self.ensure_token().await?;
        let url = format!("{FB_API_BASE}{path}");
        debug!(url = %url, "PUT");

        let resp = self
            .http
            .put(&url)
            .header("Authorization", format!("Bearer {token}"))
            .header("Content-Type", "application/json")
            .header("Api-Version", "alpha")
            .json(body)
            .send()
            .await?;

        self.handle_response(resp).await
    }

    async fn handle_response(&self, resp: reqwest::Response) -> Result<Value, ApiError> {
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(ApiError::Api { status, body });
        }
        Ok(resp.json::<Value>().await?)
    }

    fn save_token(&self, token: &TokenData) -> Result<(), ApiError> {
        if let Some(parent) = self.token_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                ApiError::Other(format!("Failed to create config dir: {e}"))
            })?;
        }
        let json = serde_json::to_string_pretty(token)
            .map_err(|e| ApiError::Other(format!("Failed to serialize token: {e}")))?;
        std::fs::write(&self.token_path, json).map_err(|e| {
            ApiError::Other(format!(
                "Failed to write token to {}: {e}",
                self.token_path.display()
            ))
        })?;
        debug!("token saved to {}", self.token_path.display());
        Ok(())
    }
}

fn now_epoch() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}
