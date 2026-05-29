use std::sync::Arc;

use crate::application::{AppError, FrameSink, LiveKitSession, SessionHandle, TokenExchange};
use crate::domain::{BearerToken, RequestBody};

pub struct PlayStreamUseCase {
    token_exchange: Arc<dyn TokenExchange>,
    livekit: Arc<dyn LiveKitSession>,
}

impl PlayStreamUseCase {
    pub fn new(token_exchange: Arc<dyn TokenExchange>, livekit: Arc<dyn LiveKitSession>) -> Self {
        Self {
            token_exchange,
            livekit,
        }
    }

    pub async fn play(
        &self,
        token: BearerToken,
        body: RequestBody,
        sink: Box<dyn FrameSink>,
    ) -> Result<SessionHandle, AppError> {
        let creds = self.token_exchange.exchange(&token, &body).await?;
        self.livekit.connect_and_stream(creds, sink).await
    }
}
