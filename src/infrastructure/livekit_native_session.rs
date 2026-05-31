use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use futures::StreamExt;
use livekit::prelude::RemoteTrackPublication;
use livekit::track::RemoteTrack;
use livekit::webrtc::prelude::VideoBuffer;
use livekit::webrtc::video_frame::{I420Buffer, VideoFrame};
use livekit::webrtc::video_stream::native::NativeVideoStream;
use livekit::{Room, RoomEvent, RoomOptions};
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

use crate::application::{AppError, FrameSink, LiveKitSession, SessionHandle, YuvFrame};
use crate::domain::LiveKitCredentials;

pub struct LiveKitNativeSession;

impl LiveKitNativeSession {
    pub fn new() -> Self {
        Self
    }
}

impl Default for LiveKitNativeSession {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl LiveKitSession for LiveKitNativeSession {
    async fn connect_and_stream(
        &self,
        creds: LiveKitCredentials,
        sink: Box<dyn FrameSink>,
    ) -> Result<SessionHandle, AppError> {
        let (room, mut events) = Room::connect(&creds.wss_url, &creds.jwt, RoomOptions::default())
            .await
            .map_err(|e| AppError::Connect(e.to_string()))?;

        log::info!("livekit room connected: {}", room.name());

        let (cancel_tx, mut cancel_rx) = oneshot::channel::<()>();
        let sink: Arc<dyn FrameSink> = Arc::from(sink);

        let task = tokio::spawn(async move {
            let mut drains = ActiveVideoDrains::default();

            loop {
                tokio::select! {
                    _ = &mut cancel_rx => {
                        log::info!("livekit session cancelled");
                        drains.unsubscribe_all().await;
                        if let Err(e) = room.close().await {
                            log::warn!("livekit room close failed: {e}");
                        }
                        break;
                    }
                    maybe_event = events.recv() => {
                        let Some(event) = maybe_event else {
                            log::info!("livekit event stream closed");
                            drains.abort_all().await;
                            break;
                        };
                        match event {
                            RoomEvent::TrackSubscribed {
                                track: RemoteTrack::Video(video),
                                publication,
                                ..
                            } => {
                                log::info!("video track subscribed: {}", video.sid());
                                let sink = sink.clone();
                                let task = tokio::spawn(drain_video(video, sink));
                                drains.insert(LiveKitVideoSubscription { publication }, task);
                            }
                            RoomEvent::TrackUnsubscribed { publication, .. } => {
                                drains.remove(&publication.sid().to_string()).await;
                            }
                            _ => {}
                        }
                    }
                }
            }
        });

        Ok(SessionHandle::new(cancel_tx, task))
    }
}

trait VideoSubscription: Send {
    fn sid(&self) -> String;
    fn unsubscribe(&self);
}

struct LiveKitVideoSubscription {
    publication: RemoteTrackPublication,
}

impl VideoSubscription for LiveKitVideoSubscription {
    fn sid(&self) -> String {
        self.publication.sid().to_string()
    }

    fn unsubscribe(&self) {
        self.publication.set_subscribed(false);
    }
}

struct ActiveVideoDrain {
    subscription: Box<dyn VideoSubscription>,
    task: JoinHandle<()>,
}

#[derive(Default)]
struct ActiveVideoDrains {
    drains: HashMap<String, ActiveVideoDrain>,
}

impl ActiveVideoDrains {
    fn insert(&mut self, subscription: impl VideoSubscription + 'static, task: JoinHandle<()>) {
        let sid = subscription.sid();
        if let Some(previous) = self.drains.remove(&sid) {
            previous.task.abort();
        }
        self.drains.insert(
            sid,
            ActiveVideoDrain {
                subscription: Box::new(subscription),
                task,
            },
        );
    }

    async fn remove(&mut self, sid: &str) {
        if let Some(drain) = self.drains.remove(sid) {
            drain.task.abort();
            let _ = drain.task.await;
        }
    }

    async fn unsubscribe_all(&mut self) {
        let drains = std::mem::take(&mut self.drains);
        for (_, drain) in drains {
            drain.subscription.unsubscribe();
            drain.task.abort();
            let _ = drain.task.await;
        }
    }

    async fn abort_all(&mut self) {
        let drains = std::mem::take(&mut self.drains);
        for (_, drain) in drains {
            drain.task.abort();
            let _ = drain.task.await;
        }
    }

    #[cfg(test)]
    fn is_empty(&self) -> bool {
        self.drains.is_empty()
    }
}

async fn drain_video(track: livekit::track::RemoteVideoTrack, sink: Arc<dyn FrameSink>) {
    let mut stream = NativeVideoStream::new(track.rtc_track());

    while let Some(VideoFrame { buffer, .. }) = stream.next().await {
        let i420 = buffer.to_i420();
        sink.submit_frame(yuv_frame_from_i420(&i420));
    }

    log::info!("video drain loop exited");
}

fn yuv_frame_from_i420(i420: &I420Buffer) -> YuvFrame {
    let (y_stride, u_stride, v_stride) = i420.strides();
    let (y_plane, u_plane, v_plane) = i420.data();

    YuvFrame {
        width: i420.width(),
        height: i420.height(),
        y_plane: y_plane.to_vec(),
        u_plane: u_plane.to_vec(),
        v_plane: v_plane.to_vec(),
        y_stride,
        u_stride,
        v_stride,
    }
}

#[cfg(test)]
mod tests {
    use super::{yuv_frame_from_i420, ActiveVideoDrains, VideoSubscription};
    use livekit::webrtc::video_frame::I420Buffer;
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    };

    struct FakeSubscription {
        sid: String,
        unsubscribed: Arc<Mutex<Vec<String>>>,
    }

    impl VideoSubscription for FakeSubscription {
        fn sid(&self) -> String {
            self.sid.clone()
        }

        fn unsubscribe(&self) {
            self.unsubscribed.lock().unwrap().push(self.sid.clone());
        }
    }

    #[tokio::test]
    async fn stop_unsubscribes_and_aborts_active_video_drains() {
        let unsubscribed = Arc::new(Mutex::new(Vec::new()));
        let dropped = Arc::new(AtomicBool::new(false));
        let dropped_for_task = dropped.clone();
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();

        let task = tokio::spawn(async move {
            started_tx.send(()).unwrap();
            let _guard = DropFlag(dropped_for_task);
            std::future::pending::<()>().await;
        });
        started_rx.await.unwrap();

        let mut drains = ActiveVideoDrains::default();
        drains.insert(
            FakeSubscription {
                sid: "track-1".to_string(),
                unsubscribed: unsubscribed.clone(),
            },
            task,
        );

        drains.unsubscribe_all().await;

        assert_eq!(*unsubscribed.lock().unwrap(), vec!["track-1".to_string()]);
        assert!(dropped.load(Ordering::SeqCst));
        assert!(drains.is_empty());
    }

    struct DropFlag(Arc<AtomicBool>);

    impl Drop for DropFlag {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }

    #[test]
    fn converts_i420_buffer_to_owned_yuv_frame() {
        let mut i420 = I420Buffer::new(4, 2);
        let (y, u, v) = i420.data_mut();
        y.copy_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8]);
        u.copy_from_slice(&[9, 10]);
        v.copy_from_slice(&[11, 12]);

        let frame = yuv_frame_from_i420(&i420);

        assert_eq!(frame.width, 4);
        assert_eq!(frame.height, 2);
        assert_eq!(frame.y_stride, 4);
        assert_eq!(frame.u_stride, 2);
        assert_eq!(frame.v_stride, 2);
        assert_eq!(frame.y_plane, vec![1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(frame.u_plane, vec![9, 10]);
        assert_eq!(frame.v_plane, vec![11, 12]);
    }
}
