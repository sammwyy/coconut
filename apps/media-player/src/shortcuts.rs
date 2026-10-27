use crate::state::PlayerState;
use creamui_render::platform::{Key, KeyEvent, Modifiers};

impl PlayerState {
    pub fn key(&self, event: KeyEvent, modifiers: Modifiers) -> bool {
        if !event.pressed || event.synthetic || modifiers.ctrl {
            return false;
        }
        let playback = self.config.peek().playback;
        let seek = if modifiers.shift {
            playback.seek_step_long
        } else {
            playback.seek_step
        } as f64;
        match &event.key {
            Key::Space => self.toggle(),
            Key::Left => self.seek_by(-seek),
            Key::Right => self.seek_by(seek),
            Key::Up => self.set_volume(self.volume.get() + 0.05),
            Key::Down => self.set_volume(self.volume.get() - 0.05),
            Key::Escape => {
                if self.is_maximized() {
                    self.toggle_maximize();
                } else if let Some(handle) = self.window.borrow().as_ref() {
                    handle.close();
                }
            }
            Key::Character(text) => {
                let ch = text.as_str();
                if ch == " " {
                    self.toggle();
                } else if ch.eq_ignore_ascii_case("f") {
                    self.toggle_maximize();
                } else if ch.eq_ignore_ascii_case("m") {
                    self.toggle_mute();
                } else if ch == "]" {
                    self.cycle_speed(true);
                } else if ch == "[" {
                    self.cycle_speed(false);
                } else if ch.eq_ignore_ascii_case("n") {
                    self.advance(true);
                } else if ch.eq_ignore_ascii_case("p") {
                    self.prev();
                } else if ch.eq_ignore_ascii_case("l") {
                    self.cycle_repeat();
                } else if ch.eq_ignore_ascii_case("s") {
                    self.toggle_shuffle();
                } else if ch.eq_ignore_ascii_case("o") {
                    self.open_dialog();
                } else if ch.eq_ignore_ascii_case("t") {
                    self.queue_open.set(!self.queue_open.get());
                } else if ch == "," {
                    crate::app::open_settings();
                } else if ch.eq_ignore_ascii_case("q") {
                    if let Some(handle) = self.window.borrow().as_ref() {
                        handle.close();
                    }
                } else {
                    return false;
                }
            }
            _ => return false,
        }
        true
    }
}
