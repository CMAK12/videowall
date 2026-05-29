mod domain;
mod application;
mod infrastructure;
mod presentation;

fn main() -> Result<(), slint::PlatformError> {
    presentation::run()
}
