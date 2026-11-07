//! System Monitor — a small, native system monitor.

mod actions;
mod app;
mod applications;
mod system;
mod telemetry;
mod ui;

fn main() {
    app::run();
}
