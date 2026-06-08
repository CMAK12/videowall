# videowall

`videowall` is a Rust desktop application for playing a LiveKit-backed video
stream in a Slint UI. The app exchanges a bearer token and JSON request body for
LiveKit credentials, connects to the LiveKit room, receives I420/YUV video
frames, converts them to RGB on the GPU with `wgpu`, and displays the result in
the window.

## Requirements

- Rust stable toolchain with Cargo.
- A desktop environment supported by Slint and `wgpu`.
- Network access to the configured token exchange endpoint.
- A valid bearer token and request body for the LiveKit stream.

On macOS, the project includes `.cargo/config.toml` with the Objective-C linker
flag required by native dependencies.

## Run

From the project root:

```sh
cargo run
```

The app opens a `videowall` window. Paste or type:

1. Token: a bearer token. The app also accepts values prefixed with `Bearer ` and
   strips that prefix before sending the request.
2. Body: the JSON request body expected by the token exchange endpoint.

Then click `Play`. Click `Stop` to close the current LiveKit session and clear
the displayed frame.

For runtime logs:

```sh
RUST_LOG=info cargo run
```

## Test and Check

Run the unit tests:

```sh
cargo test
```

Check that the project compiles without producing a binary:

```sh
cargo check
```

Format Rust sources:

```sh
cargo fmt
```

## Project Structure

```text
.
├── .cargo/
│   └── config.toml
├── Cargo.lock
├── Cargo.toml
├── README.md
├── build.rs
├── docs/
│   └── superpowers/
│       └── plans/
│           └── 2026-05-30-gpu-yuv-rendering.md
├── src/
│   ├── application/
│   │   ├── errors.rs
│   │   ├── mod.rs
│   │   ├── play_stream.rs
│   │   ├── ports.rs
│   │   └── yuv_frame.rs
│   ├── domain/
│   │   ├── credentials.rs
│   │   ├── mod.rs
│   │   └── token.rs
│   ├── infrastructure/
│   │   ├── livekit_native_session.rs
│   │   ├── mod.rs
│   │   └── reqwest_token_exchange.rs
│   ├── presentation/
│   │   ├── gpu/
│   │   │   ├── mod.rs
│   │   │   ├── yuv_pipeline.rs
│   │   │   └── yuv_to_rgb.wgsl
│   │   ├── mod.rs
│   │   └── ui_frame_sink.rs
│   └── main.rs
└── ui/
    └── app.slint
```

### Root Files

- `.cargo/config.toml`: Cargo configuration. On macOS it passes `-ObjC` to the
  linker for native Objective-C symbols used by dependencies.
- `Cargo.toml`: Crate metadata and dependencies, including Slint, Tokio,
  Reqwest, LiveKit, `wgpu` support through Slint, logging, and clipboard access.
- `Cargo.lock`: Locked dependency versions for reproducible builds.
- `build.rs`: Build script that compiles `ui/app.slint` into Rust bindings with
  `slint-build`.
- `README.md`: Project setup, run instructions, and structure documentation.

### `ui/`

- `ui/app.slint`: Slint UI definition for the main window. It defines the token
  and body inputs, paste buttons, play/stop controls, error text, and video image
  area.

### `src/main.rs`

- `src/main.rs`: Application entry point. It wires the top-level modules and
  delegates startup to the presentation layer with `presentation::run()`.

### `src/domain/`

Pure domain values and conversion rules. This layer has no dependency on the
application, infrastructure, or presentation layers.

- `src/domain/mod.rs`: Domain module exports.
- `src/domain/token.rs`: Defines `BearerToken` and `RequestBody`. `BearerToken`
  trims whitespace and removes an optional `Bearer ` prefix.
- `src/domain/credentials.rs`: Defines `LiveKitCredentials` and converts API
  `http`/`https` URLs to LiveKit `ws`/`wss` URLs.

### `src/application/`

Use cases and ports. This layer coordinates domain objects and defines traits
that infrastructure implements.

- `src/application/mod.rs`: Application module exports.
- `src/application/errors.rs`: Shared `AppError` enum for token exchange and
  LiveKit connection failures.
- `src/application/play_stream.rs`: `PlayStreamUseCase`, which exchanges the
  provided token/body for LiveKit credentials and starts streaming.
- `src/application/ports.rs`: Trait definitions for `TokenExchange`,
  `LiveKitSession`, and `FrameSink`, plus `SessionHandle` for stopping a running
  stream.
- `src/application/yuv_frame.rs`: Owned I420/YUV frame data transfer object with
  plane bytes, dimensions, and strides.

### `src/infrastructure/`

Concrete adapters for external systems and IO.

- `src/infrastructure/mod.rs`: Infrastructure module exports.
- `src/infrastructure/reqwest_token_exchange.rs`: Reqwest implementation of the
  token exchange port. It posts the request body to the configured staging API
  endpoint with bearer authentication and parses the LiveKit token response.
- `src/infrastructure/livekit_native_session.rs`: LiveKit implementation of the
  streaming port. It connects to a room, subscribes to remote video tracks,
  drains native video frames, converts them to owned `YuvFrame` values, and
  handles session cancellation.

### `src/presentation/`

Slint UI integration and GPU presentation code.

- `src/presentation/mod.rs`: Starts logging, selects the Slint `wgpu` backend,
  creates the window, wires UI callbacks, manages clipboard paste behavior, and
  starts/stops stream sessions on a Tokio runtime.
- `src/presentation/ui_frame_sink.rs`: Implements `FrameSink` for the UI. It
  renders YUV frames to `wgpu` textures and sends Slint images back onto the UI
  event loop.
- `src/presentation/gpu/mod.rs`: GPU module exports and `GpuCtx`, which stores
  the Slint-owned `wgpu` device and queue.
- `src/presentation/gpu/yuv_pipeline.rs`: `wgpu` render pipeline that uploads Y,
  U, and V planes, runs the YUV-to-RGB shader, and returns an RGBA texture for
  Slint.
- `src/presentation/gpu/yuv_to_rgb.wgsl`: WGSL shader for fullscreen-triangle
  rendering and BT.601 limited-range YUV-to-RGB conversion.

## Architecture Notes

The code is organized in layers:

- `domain`: data types and pure rules.
- `application`: use cases and interface traits.
- `infrastructure`: concrete network and LiveKit adapters.
- `presentation`: Slint UI, clipboard integration, runtime wiring, and GPU
  rendering.

This keeps LiveKit, HTTP, and UI details outside the core use case while still
allowing the presentation layer to compose the concrete adapters at startup.
