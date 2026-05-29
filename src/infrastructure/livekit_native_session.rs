use std::sync::Arc;

use async_trait::async_trait;
use futures::StreamExt;
use livekit::track::RemoteTrack;
use livekit::webrtc::prelude::VideoBuffer;
use livekit::webrtc::video_frame::native::VideoFrameBufferExt;
use livekit::webrtc::video_frame::{VideoFormatType, VideoFrame};
use livekit::webrtc::video_stream::native::NativeVideoStream;
use livekit::{Room, RoomEvent, RoomOptions};
use slint::{Rgba8Pixel, SharedPixelBuffer};
use tokio::sync::oneshot;

use crate::application::{AppError, FrameSink, LiveKitSession, SessionHandle};
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
    let mut buf: Option<SharedPixelBuffer<Rgba8Pixel>> = None;

    while let Some(VideoFrame { buffer, .. }) = stream.next().await {
        let i420 = buffer.to_i420();
        let w = i420.width();
        let h = i420.height();

        let needs_alloc = match &buf {
            Some(pb) => pb.width() != w || pb.height() != h,
            None => true,
        };
        if needs_alloc {
            buf = Some(SharedPixelBuffer::<Rgba8Pixel>::new(w, h));
        }
        let pb = buf.as_mut().expect("buffer just allocated");

        let dst_stride = w * 4;
        i420.to_argb(
            VideoFormatType::RGBA,
            pb.make_mut_bytes(),
            dst_stride,
            w as i32,
            h as i32,
        );

        sink.submit_frame(w, h, pb.as_bytes());
    }

    log::info!("video drain loop exited");
}
