use slint::{Image, Rgba8Pixel, SharedPixelBuffer, Weak};

use crate::application::FrameSink;

use super::AppWindow;

pub struct UiFrameSink {
    weak: Weak<AppWindow>,
}

impl UiFrameSink {
    pub fn new(weak: Weak<AppWindow>) -> Self {
        Self { weak }
    }
}

impl FrameSink for UiFrameSink {
    fn submit_frame(&self, width: u32, height: u32, pixels: &[u8]) {
        let mut buf = SharedPixelBuffer::<Rgba8Pixel>::new(width, height);
        let dst = buf.make_mut_bytes();
        if dst.len() != pixels.len() {
            log::warn!(
                "frame size mismatch: got {} bytes for {}x{} (expected {})",
                pixels.len(),
                width,
                height,
                dst.len()
            );
            return;
        }
        dst.copy_from_slice(pixels);

        let weak = self.weak.clone();
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(app) = weak.upgrade() {
                app.set_video_frame(Image::from_rgba8(buf));
            }
        });
    }
}
