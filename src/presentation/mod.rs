//! Presentation layer.
//!
//! Slint UI. Holds the generated component bindings and drives the
//! event loop. Calls `application` use cases; never reaches into
//! `infrastructure` directly.

use std::sync::{Arc, Mutex as StdMutex, OnceLock};

use arboard::Clipboard;
use slint::{ComponentHandle, Image, SharedString};
use tokio::runtime::Runtime;
use tokio::sync::Mutex;

use crate::application::{FrameSink, PlayStreamUseCase, SessionHandle};
use crate::domain::{BearerToken, RequestBody};
use crate::infrastructure::{LiveKitNativeSession, ReqwestTokenExchange};

slint::include_modules!();

mod gpu;
mod ui_frame_sink;
use ui_frame_sink::UiFrameSink;

pub fn run() -> Result<(), slint::PlatformError> {
    env_logger::init();

    slint::BackendSelector::new()
        .require_wgpu_28(slint::wgpu_28::WGPUConfiguration::default())
        .select()
        .map_err(|e| {
            log::error!("failed to select wgpu backend: {e}");
            slint::PlatformError::Other(format!("wgpu backend: {e}"))
        })?;

    let rt = Runtime::new().expect("tokio runtime");
    let rt_handle = rt.handle().clone();

    let app = AppWindow::new()?;
    let weak = app.as_weak();

    let gpu: Arc<OnceLock<gpu::GpuCtx>> = Arc::new(OnceLock::new());
    {
        let gpu = gpu.clone();
        app.window()
            .set_rendering_notifier(move |state, api| {
                if let (
                    slint::RenderingState::RenderingSetup,
                    slint::GraphicsAPI::WGPU28 { device, queue, .. },
                ) = (&state, &api)
                {
                    if gpu
                        .set(gpu::GpuCtx::new((*device).clone(), (*queue).clone()))
                        .is_err()
                    {
                        log::warn!("rendering notifier fired RenderingSetup twice");
                    } else {
                        log::info!("captured wgpu device/queue from Slint");
                    }
                }
            })
            .map_err(|e| slint::PlatformError::Other(format!("rendering notifier: {e}")))?;
    }
    let use_case = Arc::new(PlayStreamUseCase::new(
        Arc::new(ReqwestTokenExchange::new()),
        Arc::new(LiveKitNativeSession::new()),
    ));

    let session: Arc<Mutex<Option<SessionHandle>>> = Arc::new(Mutex::new(None));

    // Cache a single arboard::Clipboard so paste handlers don't re-init
    // the pasteboard on every click. Initialised eagerly at startup so
    // the (occasionally slow) first-touch cost happens before the user
    // can interact with the paste buttons.
    let clipboard: Arc<StdMutex<Option<Clipboard>>> =
        Arc::new(StdMutex::new(Clipboard::new().ok()));

    // Build the pipeline lazily on the first Play so that the rendering
    // notifier has had time to deliver the device. RenderingSetup fires
    // synchronously inside `app.run()` before the window is interactive,
    // so by the time the user clicks Play the device is guaranteed to
    // be present.
    let pipeline: Arc<OnceLock<Arc<StdMutex<gpu::YuvPipeline>>>> = Arc::new(OnceLock::new());

    {
        let weak = weak.clone();
        let use_case = use_case.clone();
        let session = session.clone();
        let rt_handle = rt_handle.clone();
        let gpu = gpu.clone();
        let pipeline = pipeline.clone();
        app.on_play(move |token_s, body_s| {
            let weak = weak.clone();
            let use_case = use_case.clone();
            let session = session.clone();
            let token = BearerToken::new(token_s.to_string());
            let body = RequestBody::from_string(body_s.to_string());

            let Some(gpu_ctx) = gpu.get() else {
                if let Some(app) = weak.upgrade() {
                    app.set_error_text(
                        "GPU not initialised yet — wait for the window to finish loading and retry."
                            .into(),
                    );
                }
                return;
            };
            let device = gpu_ctx.device.clone();
            let queue = gpu_ctx.queue.clone();
            let pipeline_arc = pipeline
                .get_or_init(|| Arc::new(StdMutex::new(gpu::YuvPipeline::new(&device))))
                .clone();

            if let Some(app) = weak.upgrade() {
                app.set_error_text("".into());
                app.set_is_playing(true);
            }

            rt_handle.spawn(async move {
                let sink: Box<dyn FrameSink> = Box::new(UiFrameSink::new(
                    weak.clone(),
                    device,
                    queue,
                    pipeline_arc,
                ));
                match use_case.play(token, body, sink).await {
                    Ok(handle) => {
                        *session.lock().await = Some(handle);
                    }
                    Err(e) => {
                        let msg = e.to_string();
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(app) = weak.upgrade() {
                                app.set_error_text(msg.into());
                                app.set_is_playing(false);
                            }
                        });
                    }
                }
            });
        });
    }

    {
        let weak = weak.clone();
        let clipboard = clipboard.clone();
        app.on_paste_token(move || {
            if let Some(text) = read_clipboard(&clipboard) {
                if let Some(app) = weak.upgrade() {
                    app.set_token_text(SharedString::from(text));
                }
            }
        });
    }

    {
        let weak = weak.clone();
        let clipboard = clipboard.clone();
        app.on_paste_body(move || {
            if let Some(text) = read_clipboard(&clipboard) {
                if let Some(app) = weak.upgrade() {
                    app.set_body_text(SharedString::from(text));
                }
            }
        });
    }

    {
        let weak = weak.clone();
        let session = session.clone();
        let rt_handle = rt_handle.clone();
        app.on_stop(move || {
            let weak = weak.clone();
            let session = session.clone();
            rt_handle.spawn(async move {
                if let Some(handle) = session.lock().await.take() {
                    let _ = handle.cancel.send(());
                }
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(app) = weak.upgrade() {
                        app.set_is_playing(false);
                        app.set_video_frame(Image::default());
                    }
                });
            });
        });
    }

    app.run()?;
    drop(rt);
    Ok(())
}

fn read_clipboard(cache: &StdMutex<Option<Clipboard>>) -> Option<String> {
    let mut guard = cache.lock().expect("clipboard mutex poisoned");
    if guard.is_none() {
        match Clipboard::new() {
            Ok(c) => *guard = Some(c),
            Err(e) => {
                log::warn!("clipboard init failed: {e}");
                return None;
            }
        }
    }
    match guard.as_mut()?.get_text() {
        Ok(text) => Some(text),
        Err(e) => {
            log::warn!("clipboard read failed: {e}");
            // Drop the (possibly broken) handle so the next call retries init.
            *guard = None;
            None
        }
    }
}
