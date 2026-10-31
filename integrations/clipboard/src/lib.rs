pub mod client;
pub mod config;
pub mod daemon;
mod history;
mod wayland;

pub const SERVICE: &str = "org.coconut.Clipboard";
pub const PATH: &str = "/org/coconut/Clipboard1";
pub const INTERFACE: &str = "org.coconut.Clipboard1";
pub const READ: &str = "clipboard-read";
pub const HISTORY: &str = "clipboard-history";
pub const WRITE: &str = "clipboard-write";

pub type Entry = (u64, u64, String);
pub type Status = (bool, bool, String, u64);
