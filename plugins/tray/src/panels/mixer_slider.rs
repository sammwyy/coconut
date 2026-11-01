use creamui_core::{CursorIcon, KeyInput, Painter, Point, Rect, Style, StyleState, Widget};
use creamui_widgets::RawSlider;
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Clone, Default)]
pub(super) struct MixerGesture(pub Rc<RefCell<Option<String>>>);

pub(super) struct MixerSlider {
    slider: RawSlider,
    id: String,
    gesture: MixerGesture,
}
impl MixerSlider {
    pub fn new(slider: RawSlider, id: String, gesture: MixerGesture) -> Self {
        Self {
            slider,
            id,
            gesture,
        }
    }
}
impl Widget for MixerSlider {
    fn style(&self) -> Style {
        self.slider.style()
    }
    fn style_state(&self) -> StyleState {
        self.slider.style_state()
    }
    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        self.slider.paint(painter, rect);
    }
    fn focusable(&self) -> bool {
        self.slider.focusable()
    }
    fn cursor_icon(&self) -> Option<CursorIcon> {
        self.slider.cursor_icon()
    }
    fn on_key(&self) -> Option<Rc<dyn Fn(KeyInput)>> {
        self.slider.on_key()
    }
    fn on_drag_start(&self) -> Option<Rc<dyn Fn(Point, Rect)>> {
        let gesture = self.gesture.clone();
        let id = self.id.clone();
        Some(Rc::new(move |_, _| {
            *gesture.0.borrow_mut() = Some(id.clone());
        }))
    }
    fn on_drag(&self) -> Option<Rc<dyn Fn(Point, Rect)>> {
        let drag = self.slider.on_drag()?;
        let id = self.id.clone();
        let gesture = self.gesture.clone();
        Some(Rc::new(move |point, rect| {
            let matches = gesture.0.borrow().as_deref() == Some(id.as_str());
            if matches {
                drag(point, rect);
            }
        }))
    }
    fn on_drag_end(&self) -> Option<Rc<dyn Fn()>> {
        let gesture = self.gesture.clone();
        Some(Rc::new(move || {
            gesture.0.borrow_mut().take();
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_removed_stream_cannot_redirect_an_existing_drag_to_another_application() {
        let gesture = MixerGesture::default();
        let calls = Rc::new(RefCell::new(Vec::new()));
        let make = |id: &str| {
            let calls = calls.clone();
            let name = id.to_owned();
            MixerSlider::new(
                RawSlider::new(
                    Style::new(),
                    0.5,
                    creamui_theme::Color::rgb(0, 0, 0),
                    creamui_theme::Color::rgb(0, 0, 0),
                    creamui_theme::Color::rgb(255, 255, 255),
                    move |_| calls.borrow_mut().push(name.clone()),
                ),
                id.into(),
                gesture.clone(),
            )
        };
        let first = make("first");
        let second = make("second");
        let rect = Rect {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 28.0,
        };
        let point = Point { x: 80.0, y: 10.0 };
        first.on_drag_start().unwrap()(point, rect);
        second.on_drag().unwrap()(point, rect);
        assert!(calls.borrow().is_empty());
        first.on_drag().unwrap()(point, rect);
        assert_eq!(&*calls.borrow(), &["first"]);
        first.on_drag_end().unwrap()();
        second.on_drag().unwrap()(point, rect);
        assert_eq!(calls.borrow().len(), 1);
    }
}
