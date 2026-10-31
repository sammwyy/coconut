use coconut_core::{BarEdit, DockConfig};
use coconut_plugin_kit::bar_editor::EditHandler;
use creamui_core::{BoxedWidget, Painter, Point, Rect, Style, Widget};
use creamui_reactive::Signal;
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Region {
    Viewport,
    Group(usize),
    Item { group: usize, item: usize },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Destination {
    Item { group: usize, at: usize },
    Group { at: usize },
}

struct Payload {
    source: Region,
    bar: DockConfig,
    apply: EditHandler,
}

#[derive(Clone)]
struct Drag {
    payload: Rc<Payload>,
    origin: Point,
    pointer: Point,
    started: bool,
    destination: Option<Destination>,
}

#[derive(Clone)]
pub struct State {
    active: Signal<Option<Drag>>,
    bounds: Rc<RefCell<HashMap<Region, Rect>>>,
    viewport: Rc<Cell<Option<Rect>>>,
    generation: Rc<Cell<u64>>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            active: Signal::new(None),
            bounds: Default::default(),
            viewport: Default::default(),
            generation: Default::default(),
        }
    }
}

impl State {
    pub fn begin_frame(&self) {
        self.bounds.borrow_mut().clear();
    }

    pub fn cancel(&self) {
        self.generation.set(self.generation.get().wrapping_add(1));
        self.active.set(None);
    }

    pub fn is_active(&self) -> bool {
        self.active.peek().is_some()
    }

    pub fn start(&self, source: Region, origin: Point, bar: DockConfig, apply: EditHandler) {
        self.generation.set(self.generation.get().wrapping_add(1));
        self.active.set(Some(Drag {
            payload: Rc::new(Payload { source, bar, apply }),
            origin,
            pointer: origin,
            started: false,
            destination: None,
        }));
    }

    pub fn update(&self, point: Point) {
        let Some(mut drag) = self.active.peek() else {
            return;
        };
        drag.pointer = point;
        drag.started |= (point.x - drag.origin.x).hypot(point.y - drag.origin.y) >= 4.0;
        drag.destination = drag.started.then(|| self.resolve(&drag)).flatten();
        self.active.set(Some(drag));
    }

    pub fn finish(&self) -> Option<usize> {
        let drag = self.active.peek()?;
        let destination = drag.started.then(|| self.resolve(&drag)).flatten();
        self.cancel();
        let (edit, group) = match (drag.payload.source, destination?) {
            (
                Region::Item { group, item },
                Destination::Item {
                    group: to_group,
                    at,
                },
            ) => {
                if group == to_group && (at == item || at == item + 1) {
                    return None;
                }
                (
                    BarEdit::MoveItem {
                        group,
                        item,
                        to_group,
                        at,
                    },
                    to_group,
                )
            }
            (Region::Group(from), Destination::Group { at }) => {
                let to = if at > from { at - 1 } else { at };
                if to == from {
                    return None;
                }
                (BarEdit::MoveGroup { from, to }, to)
            }
            _ => return None,
        };
        (drag.payload.apply)(edit);
        Some(group)
    }

    fn resolve(&self, drag: &Drag) -> Option<Destination> {
        if !self.viewport.get()?.contains(drag.pointer) {
            return None;
        }
        let bounds = self.bounds.borrow();
        if let Region::Item {
            group: source_group,
            ..
        } = drag.payload.source
        {
            for (region, rect) in bounds.iter() {
                if let Region::Item { group, item } = region {
                    if rect.contains(drag.pointer) {
                        return Some(Destination::Item {
                            group: *group,
                            at: item + usize::from(drag.pointer.y >= rect.y + rect.height / 2.0),
                        });
                    }
                }
            }
            for (region, rect) in bounds.iter() {
                if let Region::Group(group) = region {
                    if *group != source_group && rect.contains(drag.pointer) {
                        return Some(Destination::Item {
                            group: *group,
                            at: drag.payload.bar.sections.get(*group)?.islands.len(),
                        });
                    }
                }
            }
        } else if matches!(drag.payload.source, Region::Group(_)) {
            for (region, rect) in bounds.iter() {
                if let Region::Group(group) = region {
                    if rect.contains(drag.pointer) {
                        return Some(Destination::Group {
                            at: group + usize::from(drag.pointer.x >= rect.x + rect.width / 2.0),
                        });
                    }
                }
            }
        }
        None
    }

    pub fn destination(&self) -> Option<Destination> {
        self.active.get().and_then(|drag| drag.destination)
    }

    pub fn source(&self) -> Option<Region> {
        self.active
            .get()
            .filter(|drag| drag.started)
            .map(|drag| drag.payload.source)
    }

    pub fn preview(&self) -> Option<(Point, String)> {
        let drag = self.active.get().filter(|drag| drag.started)?;
        let label = match drag.payload.source {
            Region::Item { group, item } => {
                let entry =
                    coconut_core::ordered_islands(&drag.payload.bar.sections.get(group)?.islands)
                        .get(item)
                        .copied()?;
                coconut_plugin_kit::bar_editor::item_label(&entry.id).to_owned()
            }
            Region::Group(group) => format!("Group {}", group + 1),
            Region::Viewport => return None,
        };
        Some((drag.pointer, label))
    }

    pub fn autoscroll(
        &self,
        app: creamui_render::AppHandle,
        scroll: creamui_widgets::ScrollController,
    ) {
        self.scroll_tick(app, scroll, self.generation.get());
    }

    fn scroll_tick(
        &self,
        app: creamui_render::AppHandle,
        scroll: creamui_widgets::ScrollController,
        generation: u64,
    ) {
        let state = self.clone();
        let next_app = app.clone();
        app.spawn_background(
            || std::thread::sleep(std::time::Duration::from_millis(30)),
            move |_| {
                if state.generation.get() != generation || !state.is_active() {
                    return;
                }
                let delta = state.scroll_direction();
                if delta != 0.0 {
                    scroll.set(scroll.offset() + delta);
                    if let Some(drag) = state.active.peek() {
                        state.update(drag.pointer);
                    }
                }
                state.scroll_tick(next_app, scroll, generation);
            },
        );
    }

    pub fn scroll_direction(&self) -> f32 {
        let Some(drag) = self.active.peek().filter(|drag| drag.started) else {
            return 0.0;
        };
        let Some(viewport) = self
            .viewport
            .get()
            .filter(|rect| rect.contains(drag.pointer))
        else {
            return 0.0;
        };
        if drag.pointer.y < viewport.y + 28.0 {
            -10.0
        } else if drag.pointer.y > viewport.y + viewport.height - 28.0 {
            10.0
        } else {
            0.0
        }
    }

    #[cfg(test)]
    pub fn rect(&self, region: Region) -> Option<Rect> {
        self.bounds.borrow().get(&region).copied()
    }
}

pub fn measured(
    state: &State,
    region: Region,
    style: Style,
    child: BoxedWidget,
    key: String,
) -> BoxedWidget {
    Box::new(Measured {
        key: creamui_core::WidgetKey::from(key),
        accent: creamui_theme::use_theme().colors.accent,
        state: state.clone(),
        region,
        style,
        child: Some(child),
    })
}

struct Measured {
    key: creamui_core::WidgetKey,
    accent: creamui_theme::Color,
    state: State,
    region: Region,
    style: Style,
    child: Option<BoxedWidget>,
}

impl Widget for Measured {
    fn key(&self) -> Option<creamui_core::WidgetKey> {
        Some(self.key.clone())
    }
    fn style(&self) -> Style {
        self.style.clone()
    }
    fn children(&mut self) -> Vec<BoxedWidget> {
        let mut children = self.child.take().into_iter().collect::<Vec<_>>();
        if self.region != Region::Viewport {
            children.push(Box::new(Marker {
                state: self.state.clone(),
                region: self.region,
                accent: self.accent,
            }));
        }
        children
    }
    fn reports_layout(&self) -> bool {
        true
    }
    fn on_layout(&self, rect: Rect) -> bool {
        if self.region == Region::Viewport {
            self.state.viewport.set(Some(rect));
        } else {
            self.state.bounds.borrow_mut().insert(self.region, rect);
        }
        false
    }
    fn paint(&self, _: &mut dyn Painter, _: Rect) {}
}

struct Marker {
    state: State,
    region: Region,
    accent: creamui_theme::Color,
}

impl Widget for Marker {
    fn style(&self) -> Style {
        let mut style = Style::new()
            .width(creamui_core::LengthValue::Percent(100.0))
            .height(creamui_core::LengthValue::Percent(100.0))
            .position(creamui_core::layout::Position::Absolute);
        style.layout.inset.left = creamui_core::layout::LengthPercentageAuto::Length(0.0);
        style.layout.inset.top = creamui_core::layout::LengthPercentageAuto::Length(0.0);
        style
    }
    fn is_portal(&self) -> bool {
        false
    }
    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        let Some(drag) = self.state.active.peek().filter(|drag| drag.started) else {
            return;
        };
        let accent = self.accent;
        let boundary = match (self.region, drag.destination) {
            (Region::Item { group, item }, Some(Destination::Item { group: target, at }))
                if group == target && (at == item || at == item + 1) =>
            {
                Rect {
                    x: rect.x + 12.0,
                    y: if at == item {
                        rect.y
                    } else {
                        rect.y + rect.height - 3.0
                    },
                    width: (rect.width - 24.0).max(0.0),
                    height: 3.0,
                }
            }
            (Region::Group(group), Some(Destination::Group { at }))
                if at == group || at == group + 1 =>
            {
                Rect {
                    x: if at == group {
                        rect.x
                    } else {
                        rect.x + rect.width - 3.0
                    },
                    y: rect.y,
                    width: 3.0,
                    height: rect.height,
                }
            }
            _ => return,
        };
        painter.fill_rect(boundary, accent, 1.5);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use coconut_core::{IslandEntry, SectionConfig};

    #[test]
    fn drops_respect_clipping_and_preserve_hidden_group_and_instance_settings() {
        creamui_reactive::with_context_scope(|| {
            let mut original = DockConfig::default();
            original.sections[0].islands =
                vec![IslandEntry::with_id("clock"), IslandEntry::with_id("clock")];
            original.sections[0].islands[1]
                .config
                .insert("format".into(), "%H:%M:%S".into());
            original.sections.push(SectionConfig {
                enabled: false,
                ..Default::default()
            });
            let bar = Rc::new(RefCell::new(original.clone()));
            let changed = bar.clone();
            let apply: EditHandler =
                Rc::new(move |edit| edit.apply(&mut changed.borrow_mut()).unwrap());
            let state = State::default();
            state.viewport.set(Some(Rect {
                x: 0.0,
                y: 100.0,
                width: 400.0,
                height: 300.0,
            }));
            state.bounds.borrow_mut().insert(
                Region::Group(1),
                Rect {
                    x: 150.0,
                    y: 20.0,
                    width: 100.0,
                    height: 120.0,
                },
            );
            let source = Region::Item { group: 0, item: 1 };
            let origin = Point { x: 20.0, y: 200.0 };
            state.start(source, origin, original.clone(), apply.clone());
            state.update(Point { x: 200.0, y: 50.0 });
            assert!(
                state.finish().is_none(),
                "clipped group area is not a drop target"
            );
            assert_eq!(*bar.borrow(), original);
            state.start(source, origin, original.clone(), apply);
            state.update(Point { x: 200.0, y: 399.0 });
            assert_eq!(state.scroll_direction(), 10.0);
            state.update(Point { x: 200.0, y: 115.0 });
            assert_eq!(
                *bar.borrow(),
                original,
                "preview must not change the configuration"
            );
            assert_eq!(state.finish(), Some(1));
            let bar = bar.borrow();
            assert!(!bar.sections[1].enabled);
            assert_eq!(bar.sections[0].islands.len(), 1);
            assert_eq!(
                bar.sections[1].islands[0].config["format"].as_str(),
                Some("%H:%M:%S")
            );
        });
    }
}
