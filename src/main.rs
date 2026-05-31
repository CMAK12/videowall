mod application;
mod domain;
mod infrastructure;
mod presentation;

fn main() -> Result<(), slint::PlatformError> {
    presentation::run()
}
