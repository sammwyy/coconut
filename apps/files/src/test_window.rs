use creamui_core::{Rect, TitlebarMode, TitlebarState, WindowControls, WindowControlsBackend};
use std::{cell::Cell, rc::Rc};

pub(crate) struct TestWindow {
    state: Cell<TitlebarState>,
}

struct TestBackend(Rc<TestWindow>);

impl WindowControlsBackend for TestBackend {
    fn state(&self) -> TitlebarState {
        self.0.state.get()
    }

    fn request_titlebar(&self) {}
    fn minimize(&self) {}
    fn toggle_maximize(&self) {}
    fn close(&self) {}
}

impl TestWindow {
    pub(crate) fn install(state: TitlebarState) -> Rc<Self> {
        let window = Rc::new(Self {
            state: Cell::new(state),
        });
        creamui_reactive::provide_context(WindowControls::new(TestBackend(window.clone())));
        window
    }

    pub(crate) fn set(&self, mode: TitlebarMode, overlay: Rect) {
        self.state.set(TitlebarState {
            mode,
            overlay,
            ..Default::default()
        });
    }
}
