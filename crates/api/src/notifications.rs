use std::sync::{mpsc, Arc, Mutex};

#[derive(Clone, Debug, PartialEq)]
pub struct NotificationImage {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Notification {
    pub id: u32,
    pub app_name: String,
    pub icon: String,
    pub image: Option<NotificationImage>,
    pub summary: String,
    pub body: String,
    pub urgency: u8,
    pub actions: Vec<(String, String)>,
    pub resident: bool,
    pub expire_timeout: i32,
}

pub enum NotificationEvent {
    Show(Notification),
    Close(u32),
}

#[derive(Clone)]
pub struct NotificationListener(pub Arc<Mutex<mpsc::Receiver<NotificationEvent>>>);

impl NotificationListener {
    pub fn wait(&self) -> Option<NotificationEvent> {
        self.0.lock().ok()?.recv().ok()
    }
}

pub trait NotificationsIntegration {
    fn start(&self) -> Option<NotificationListener>;
    fn close(&self, id: u32, reason: u32);
    fn invoke(&self, id: u32, action: &str);
}

pub struct Fallback;

impl NotificationsIntegration for Fallback {
    fn start(&self) -> Option<NotificationListener> {
        None
    }
    fn close(&self, _: u32, _: u32) {}
    fn invoke(&self, _: u32, _: &str) {}
}
