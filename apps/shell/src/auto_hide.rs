use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Duration,
};

use coconut_core::DockPosition;
use coconut_plugin_kit::PanelHost;
use creamui_reactive::{create_effect, Effect, Signal};
use creamui_render::{AppHandle, WindowHandle};

const STRIP: u32 = 2;

#[derive(Default)]
struct HideTimer {
    inside: bool,
    generation: u64,
}

impl HideTimer {
    fn cancel(&mut self) -> u64 {
        self.generation = self.generation.wrapping_add(1);
        self.generation
    }
    fn can_hide(&self, token: u64, panel_open: bool) -> bool {
        self.generation == token && !self.inside && !panel_open
    }
}

pub struct AutoHide {
    pub expanded: Signal<bool>,
    timer: RefCell<HideTimer>,
    window: RefCell<Option<WindowHandle>>,
    effect: RefCell<Option<Effect>>,
    enabled: bool,
    full_size: (u32, u32),
    position: DockPosition,
    app: AppHandle,
    panels: Rc<PanelHost>,
    pending: Cell<bool>,
}

impl AutoHide {
    pub fn new(
        enabled: bool,
        full_size: (u32, u32),
        position: DockPosition,
        app: AppHandle,
        panels: Rc<PanelHost>,
    ) -> Rc<Self> {
        Rc::new(Self {
            expanded: Signal::new(!enabled),
            timer: RefCell::new(HideTimer::default()),
            window: RefCell::new(None),
            effect: RefCell::new(None),
            enabled,
            full_size,
            position,
            app,
            panels,
            pending: Cell::new(false),
        })
    }

    pub fn initial_size(&self) -> (u32, u32) {
        if !self.enabled {
            self.full_size
        } else if self.position.is_vertical() {
            (STRIP, self.full_size.1)
        } else {
            (self.full_size.0, STRIP)
        }
    }

    pub fn install(self: &Rc<Self>, window: WindowHandle) {
        if !self.enabled {
            return;
        }
        *self.window.borrow_mut() = Some(window.clone());
        let weak = Rc::downgrade(self);
        window.on_pointer_presence(move |inside| {
            if let Some(state) = weak.upgrade() {
                let changed = {
                    let mut timer = state.timer.borrow_mut();
                    let changed = timer.inside != inside;
                    timer.inside = inside;
                    changed
                };
                if changed {
                    state.refresh();
                }
            }
        });
        let weak = Rc::downgrade(self);
        *self.effect.borrow_mut() = Some(create_effect(move || {
            if let Some(state) = weak.upgrade() {
                state.refresh();
            }
        }));
    }

    fn refresh(self: &Rc<Self>) {
        let panel_open = self.panels.has_open_panel();
        if self.timer.borrow().inside || panel_open {
            self.timer.borrow_mut().cancel();
            self.pending.set(false);
            if !self.expanded.peek() {
                self.expanded.set(true);
                if let Some(window) = self.window.borrow().as_ref() {
                    window.resize(self.full_size.0, self.full_size.1);
                }
            }
            return;
        }
        if !self.expanded.peek() || self.pending.replace(true) {
            return;
        }
        let token = self.timer.borrow_mut().cancel();
        let weak = Rc::downgrade(self);
        self.app.spawn_background(
            || std::thread::sleep(Duration::from_millis(350)),
            move |()| {
                let Some(state) = weak.upgrade() else {
                    return;
                };
                if state
                    .timer
                    .borrow()
                    .can_hide(token, state.panels.has_open_panel())
                {
                    state.pending.set(false);
                    if let Some(window) = state
                        .window
                        .borrow()
                        .as_ref()
                        .filter(|window| window.is_open())
                    {
                        state.expanded.set(false);
                        let size = state.initial_size();
                        window.resize(size.0, size.1);
                    }
                }
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reentry_and_popups_prevent_a_stale_hide_timer() {
        let mut timer = HideTimer::default();
        let first = timer.cancel();
        assert!(timer.can_hide(first, false));
        timer.inside = true;
        timer.cancel();
        assert!(!timer.can_hide(first, false));
        timer.inside = false;
        let next = timer.cancel();
        assert!(!timer.can_hide(next, true));
        assert!(timer.can_hide(next, false));
    }
}
