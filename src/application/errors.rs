use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("token exchange failed: {0}")]
    TokenExchange(String),

    #[error("livekit connect failed: {0}")]
    Connect(String),
}
