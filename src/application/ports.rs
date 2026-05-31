use async_trait::async_trait;
use tokio::sync::oneshot;

use crate::application::{AppError, YuvFrame};
use crate::domain::{BearerToken, LiveKitCredentials, RequestBody};

#[async_trait]
pub trait TokenExchange: Send + Sync + 'static {
    async fn exchange(
        &self,
        token: &BearerToken,
        body: &RequestBody,
    ) -> Result<LiveKitCredentials, AppError>;
}

pub trait FrameSink: Send + Sync + 'static {
    fn submit_frame(&self, frame: YuvFrame);
}

#[async_trait]
pub trait LiveKitSession: Send + Sync + 'static {
    async fn connect_and_stream(
        &self,
        creds: LiveKitCredentials,
        sink: Box<dyn FrameSink>,
    ) -> Result<SessionHandle, AppError>;
}

pub struct SessionHandle {
    pub cancel: oneshot::Sender<()>,
}
