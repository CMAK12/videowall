use async_trait::async_trait;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

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
    stop_tx: Option<oneshot::Sender<()>>,
    task: JoinHandle<()>,
}

impl SessionHandle {
    pub fn new(stop_tx: oneshot::Sender<()>, task: JoinHandle<()>) -> Self {
        Self {
            stop_tx: Some(stop_tx),
            task,
        }
    }

    pub async fn stop(mut self) {
        if let Some(stop_tx) = self.stop_tx.take() {
            let _ = stop_tx.send(());
        }
        if let Err(e) = self.task.await {
            log::warn!("session task ended unexpectedly: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::SessionHandle;
    use tokio::sync::oneshot;

    #[tokio::test]
    async fn stop_waits_for_session_task_to_finish_cleanup() {
        let (stop_tx, stop_rx) = oneshot::channel();
        let (cleanup_tx, cleanup_rx) = oneshot::channel();
        let (release_tx, release_rx) = oneshot::channel();
        let task = tokio::spawn(async move {
            let _ = stop_rx.await;
            cleanup_tx.send(()).unwrap();
            let _ = release_rx.await;
        });
        let handle = SessionHandle::new(stop_tx, task);

        let stop_task = tokio::spawn(handle.stop());
        cleanup_rx.await.unwrap();

        assert!(!stop_task.is_finished());
        release_tx.send(()).unwrap();
        stop_task.await.unwrap();
    }
}
