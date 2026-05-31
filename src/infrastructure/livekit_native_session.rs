use std::sync::Arc;

use async_trait::async_trait;
use futures::StreamExt;
use livekit::track::RemoteTrack;
use livekit::webrtc::prelude::VideoBuffer;
use livekit::webrtc::video_frame::VideoFrame;
use livekit::webrtc::video_stream::native::NativeVideoStream;
use livekit::{Room, RoomEvent, RoomOptions};
use tokio::sync::oneshot;

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

        tokio::spawn(async move {
            let _room_guard = room;

            loop {
                tokio::select! {
                    _ = &mut cancel_rx => {
                        log::info!("livekit session cancelled");
                        break;
                    }
                    maybe_event = events.recv() => {
                        let Some(event) = maybe_event else {
                            log::info!("livekit event stream closed");
                            break;
                        };
                        if let RoomEvent::TrackSubscribed { track: RemoteTrack::Video(video), .. } = event {
                            log::info!("video track subscribed: {}", video.sid());
                            let sink = sink.clone();
                            tokio::spawn(drain_video(video, sink));
                        }
                    }
                }
            }
        });

        Ok(SessionHandle { cancel: cancel_tx })
    }
}

async fn drain_video(track: livekit::track::RemoteVideoTrack, sink: Arc<dyn FrameSink>) {
    let mut stream = NativeVideoStream::new(track.rtc_track());

    while let Some(VideoFrame { buffer, .. }) = stream.next().await {
        let i420 = buffer.to_i420();
        let width = i420.width();
        let height = i420.height();
        let (stride_y, stride_u, stride_v) = i420.strides();
        let (y, u, v) = i420.data();

        let frame = YuvFrame {
            width,
            height,
            y_plane: y.to_vec(),
            u_plane: u.to_vec(),
            v_plane: v.to_vec(),
            y_stride: stride_y,
            u_stride: stride_u,
            v_stride: stride_v,
        };

        sink.submit_frame(frame);
    }

    log::info!("video drain loop exited");
}
