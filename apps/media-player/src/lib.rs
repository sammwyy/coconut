pub mod app;
mod artwork;
pub mod backends;
pub mod cli;
mod components;
pub mod config;
mod discord;
mod engine;
mod icons;
mod media_controls;
mod settings;
mod shortcuts;
mod state;
mod views;

pub const MEDIA_EXTENSIONS: &[&str] = &[
    "mp4", "m4v", "mkv", "webm", "avi", "mov", "ts", "mts", "m2ts", "mpg", "mpeg", "wmv", "flv",
    "3gp", "ogv", "mp3", "flac", "ogg", "oga", "opus", "m4a", "aac", "wav", "wv", "aiff", "aif",
    "wma",
];

pub use app::run;
