use async_trait::async_trait;
use reqwest::{header, Client};
use serde::Deserialize;

use crate::application::{AppError, TokenExchange};
use crate::domain::{BearerToken, LiveKitCredentials, RequestBody};

const ENDPOINT: &str = "https://api-staging.lumix.ai/v1/live-view/lwebrtc/create-token/65687f0364d1bb3b7b207c5c/6953cff92a13ade0364679ec/6953ecc355947949135d3e08";

#[derive(Deserialize)]
struct ApiResponse {
    token: String,
    #[serde(rename = "apiUrl")]
    api_url: String,
}

pub struct ReqwestTokenExchange {
    client: Client,
}

impl ReqwestTokenExchange {
    pub fn new() -> Self {
        let client = Client::builder()
            .user_agent(concat!("videowall/", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("reqwest client");
        Self { client }
    }
}

impl Default for ReqwestTokenExchange {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TokenExchange for ReqwestTokenExchange {
    async fn exchange(
        &self,
        token: &BearerToken,
        body: &RequestBody,
    ) -> Result<LiveKitCredentials, AppError> {
        let t = token.as_str();
        let preview = format!(
            "{}...{}",
            &t[..t.len().min(8)],
            &t[t.len().saturating_sub(8)..]
        );
        log::info!(
            "POST {} token(len={},preview={}) body(len={})",
            ENDPOINT,
            t.len(),
            preview,
            body.as_bytes().len()
        );

        let response = self
            .client
            .post(ENDPOINT)
            .bearer_auth(t)
            .header(header::CONTENT_TYPE, "application/json")
            .body(body.as_bytes().to_vec())
            .send()
            .await
            .map_err(|e| AppError::TokenExchange(e.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            let server_body = response.text().await.unwrap_or_default();
            return Err(AppError::TokenExchange(format!(
                "HTTP {status}: {server_body}"
            )));
        }

        let parsed: ApiResponse = response
            .json()
            .await
            .map_err(|e| AppError::TokenExchange(format!("invalid response: {e}")))?;

        Ok(LiveKitCredentials::from_api_response(
            parsed.api_url,
            parsed.token,
        ))
    }
}
