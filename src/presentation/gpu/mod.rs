//! GPU pipeline for YUV→RGB conversion.
//!
//! Uses the same `wgpu::Device` Slint owns (acquired via the rendering
//! notifier) so output textures can be handed back to Slint as `Image`.

use std::sync::Arc;

use slint::wgpu_28::wgpu;

pub mod yuv_pipeline;
pub use yuv_pipeline::YuvPipeline;

pub struct GpuCtx {
    pub device: Arc<wgpu::Device>,
    pub queue: Arc<wgpu::Queue>,
}

impl GpuCtx {
    pub fn new(device: wgpu::Device, queue: wgpu::Queue) -> Self {
        Self {
            device: Arc::new(device),
            queue: Arc::new(queue),
        }
    }
}
