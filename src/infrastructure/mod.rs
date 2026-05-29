//! Infrastructure layer.
//!
//! Concrete adapters implementing `application` ports — IO, persistence,
//! external services, OS calls. Depends on `application` and `domain`.

pub mod livekit_native_session;
pub mod reqwest_token_exchange;

pub use livekit_native_session::LiveKitNativeSession;
pub use reqwest_token_exchange::ReqwestTokenExchange;
