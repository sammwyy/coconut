//! Reusable filesystem browser shared by the app and its portal backend.

pub mod app;
mod browser;
pub mod cli;
mod components;
pub mod config;
pub mod context;
mod filesystem;
mod icon_theme;
mod icons;
mod navigation;
pub mod portal;
mod settings;
mod shortcuts;
mod views;
pub mod window;

pub use app::{show_settings, show_window};
pub use window::{WindowMode, WindowRequest};

#[cfg(test)]
mod installation_tests;
#[cfg(test)]
mod test_window;
