//! System Monitor — a small, native Linux system monitor.

mod actions;
mod app;
mod applications;
mod system;
mod telemetry;
mod ui;

fn main() {
    app::run();
}
