use super::State;
use creamui_core::{BoxedWidget, Painter, Point, Rect, Size, Style, Widget};
use creamui_reactive::Signal;
use std::{cell::Cell, rc::Rc};

#[derive(Clone)]
pub(super) struct Scroll {
    offset: Signal<f32>,
    extent: Rc<Cell<f32>>,
}

impl Default for Scroll {
    fn default() -> Self {
        Self {
            offset: Signal::new(0.0),
            extent: Default::default(),
        }
    }
}

pub(super) struct Strip {
    state: State,
    group: usize,
    vertical: bool,
    size: Size,
    scroll: Scroll,
    child: Option<BoxedWidget>,
}

impl Strip {
    pub fn wrap(
        state: &State,
        group: usize,
        vertical: bool,
        size: Size,
        child: BoxedWidget,
    ) -> BoxedWidget {
        let scroll = state.strips.borrow_mut().entry(group).or_default().clone();
        scroll.offset.get();
        Box::new(Self {
            state: state.clone(),
            group,
            vertical,
            size,
            scroll,
            child: Some(child),
        })
    }
}

impl Widget for Strip {
    fn key(&self) -> Option<creamui_core::WidgetKey> {
        Some(format!("quick-strip-{}", self.group).into())
    }
    fn style(&self) -> Style {
        let mut style = Style::new()
            .width(self.size.width)
            .height(self.size.height)
            .flex_shrink(0.0);
        style.layout.flex_direction = if self.vertical {
            creamui_core::layout::FlexDirection::Row
        } else {
            creamui_core::layout::FlexDirection::Column
        };
        let align =
            self.state
                .draft
                .peek()
                .map_or(creamui_core::layout::AlignItems::Start, |draft| {
                    use creamui_core::layout::AlignItems;
                    if self.group == usize::MAX {
                        let count = draft.current.sections.len().max(1) as f32;
                        let available = if self.vertical {
                            self.size.height
                        } else {
                            self.size.width
                        };
                        if count * if self.vertical { 120.0 } else { 104.0 }
                            + draft.current.section_gap.max(4.0) * (count - 1.0)
                            > available
                        {
                            return AlignItems::Start;
                        }
                        return match draft.current.align {
                            coconut_core::DockAlign::Start => AlignItems::Start,
                            coconut_core::DockAlign::Center => AlignItems::Center,
                            coconut_core::DockAlign::End => AlignItems::End,
                        };
                    }
                    if self.scroll.extent.get() > 0.5 {
                        return AlignItems::Start;
                    }
                    let count = draft.current.sections.len();
                    if count == 1 {
                        AlignItems::Center
                    } else if self.group == 0 {
                        AlignItems::Start
                    } else if self.group + 1 == count {
                        AlignItems::End
                    } else {
                        AlignItems::Center
                    }
                });
        style.layout.align_items = Some(align);
        style
    }
    fn children(&mut self) -> Vec<BoxedWidget> {
        vec![Box::new(Content {
            vertical: self.vertical,
            length: if self.vertical {
                self.size.height
            } else {
                self.size.width
            },
            scroll: self.scroll.clone(),
            child: self.child.take(),
        })]
    }
    fn clips_children(&self) -> bool {
        true
    }
    fn scroll_offset(&self) -> Point {
        let offset = self.scroll.offset.peek().min(self.scroll.extent.get());
        if self.vertical {
            Point { x: 0.0, y: offset }
        } else {
            Point { x: offset, y: 0.0 }
        }
    }
    fn on_scroll(&self) -> Option<Rc<dyn Fn(f32)>> {
        if self.scroll.extent.get() <= 0.5 {
            return None;
        }
        let scroll = self.scroll.clone();
        Some(Rc::new(move |delta| {
            scroll
                .offset
                .set((scroll.offset.peek() + delta).clamp(0.0, scroll.extent.get()))
        }))
    }
    fn scroll_requires_layout(&self) -> bool {
        true
    }
    fn reports_layout(&self) -> bool {
        true
    }
    fn on_layout(&self, rect: Rect) -> bool {
        self.state.clips.borrow_mut().insert(self.group, rect);
        false
    }
    fn paint(&self, _: &mut dyn Painter, _: Rect) {}
}

struct Content {
    vertical: bool,
    length: f32,
    scroll: Scroll,
    child: Option<BoxedWidget>,
}
impl Widget for Content {
    fn style(&self) -> Style {
        Style::new().flex_shrink(0.0)
    }
    fn children(&mut self) -> Vec<BoxedWidget> {
        self.child.take().into_iter().collect()
    }
    fn reports_layout(&self) -> bool {
        true
    }
    fn on_layout(&self, rect: Rect) -> bool {
        let extent = ((if self.vertical {
            rect.height
        } else {
            rect.width
        }) - self.length)
            .max(0.0);
        let changed = self.scroll.extent.replace(extent) != extent;
        if self.scroll.offset.peek() > extent {
            self.scroll.offset.set(extent);
        }
        changed
    }
    fn paint(&self, _: &mut dyn Painter, _: Rect) {}
}
