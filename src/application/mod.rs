//! Application layer.
//!
//! Use cases orchestrating domain logic. Defines ports (traits) that
//! infrastructure implements. Depends only on `domain`.

pub mod errors;
pub mod play_stream;
pub mod ports;

pub use errors::AppError;
pub use play_stream::PlayStreamUseCase;
pub use ports::{FrameSink, LiveKitSession, SessionHandle, TokenExchange};
