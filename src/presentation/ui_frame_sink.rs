use std::sync::{Arc, Mutex};

use slint::wgpu_28::wgpu;
use slint::{Image, Weak};

use crate::application::{FrameSink, YuvFrame};
use crate::presentation::gpu::YuvPipeline;

use super::AppWindow;

pub struct UiFrameSink {
    weak: Weak<AppWindow>,
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    pipeline: Arc<Mutex<YuvPipeline>>,
}

impl UiFrameSink {
    pub fn new(
        weak: Weak<AppWindow>,
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        pipeline: Arc<Mutex<YuvPipeline>>,
    ) -> Self {
        Self { weak, device, queue, pipeline }
    }
}

impl FrameSink for UiFrameSink {
    fn submit_frame(&self, frame: YuvFrame) {
        // `wgpu::Texture` is Send; do the render on the calling (LiveKit)
        // thread. `slint::Image` is *not* Send, so it must be constructed
        // on the UI thread inside the event-loop closure.
        let texture = {
            let mut p = self.pipeline.lock().expect("pipeline poisoned");
            p.render(&self.device, &self.queue, &frame)
        };
        let weak = self.weak.clone();
        let _ = slint::invoke_from_event_loop(move || {
            let image = match Image::try_from(texture) {
                Ok(img) => img,
                Err(e) => {
                    log::error!("failed to wrap wgpu texture into Image: {e}");
                    return;
                }
            };
            if let Some(app) = weak.upgrade() {
                app.set_video_frame(image);
            }
        });
    }
}
