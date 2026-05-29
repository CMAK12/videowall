//! Presentation layer.
//!
//! Slint UI. Holds the generated component bindings and drives the
//! event loop. Calls `application` use cases; never reaches into
//! `infrastructure` directly.

use std::sync::Arc;

use arboard::Clipboard;
use slint::{ComponentHandle, Image, SharedString};
use tokio::runtime::Runtime;
use tokio::sync::Mutex;

use crate::application::{FrameSink, PlayStreamUseCase, SessionHandle};
use crate::domain::{BearerToken, RequestBody};
use crate::infrastructure::{LiveKitNativeSession, ReqwestTokenExchange};

slint::include_modules!();

mod ui_frame_sink;
use ui_frame_sink::UiFrameSink;

pub fn run() -> Result<(), slint::PlatformError> {
    env_logger::init();

    let rt = Runtime::new().expect("tokio runtime");
    let rt_handle = rt.handle().clone();

    let app = AppWindow::new()?;
    let weak = app.as_weak();

    let use_case = Arc::new(PlayStreamUseCase::new(
        Arc::new(ReqwestTokenExchange::new()),
        Arc::new(LiveKitNativeSession::new()),
    ));

    let session: Arc<Mutex<Option<SessionHandle>>> = Arc::new(Mutex::new(None));

    {
        let weak = weak.clone();
        let use_case = use_case.clone();
        let session = session.clone();
        let rt_handle = rt_handle.clone();
        app.on_play(move |token_s, body_s| {
            let weak = weak.clone();
            let use_case = use_case.clone();
            let session = session.clone();
            let token = BearerToken::new(token_s.to_string());
            let body = RequestBody::from_string(body_s.to_string());

            if let Some(app) = weak.upgrade() {
                app.set_error_text("".into());
                app.set_is_playing(true);
            }

            rt_handle.spawn(async move {
                let sink: Box<dyn FrameSink> = Box::new(UiFrameSink::new(weak.clone()));
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
        app.on_paste_token(move || {
            if let Some(text) = read_clipboard() {
                if let Some(app) = weak.upgrade() {
                    app.set_token_text(SharedString::from(text));
                }
            }
        });
    }

    {
        let weak = weak.clone();
        app.on_paste_body(move || {
            if let Some(text) = read_clipboard() {
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

fn read_clipboard() -> Option<String> {
    match Clipboard::new().and_then(|mut c| c.get_text()) {
        Ok(text) => Some(text),
        Err(e) => {
            log::warn!("clipboard read failed: {e}");
            None
        }
    }
}
