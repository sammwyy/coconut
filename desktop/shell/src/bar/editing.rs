use coconut_core::{BarKind, DockConfig};
use coconut_plugin_kit::bar_editor::{self, BarEditor, Target};
use creamui_core::{BoxedWidget, Point, Size};
use creamui_render::WindowHandle;
use creamui_widgets::{layout::Flex, RawView};
use std::{cell::RefCell, rc::Rc};

#[derive(Clone)]
pub struct Editors(pub [BarEditor; 2], pub [super::quick::State; 2]);

#[derive(Clone)]
pub struct Context {
    pub editor: BarEditor,
    pub window: Rc<RefCell<Option<WindowHandle>>>,
    pub kind: BarKind,
    pub bar: DockConfig,
    pub quick: super::quick::State,
    pub available: Rc<Vec<String>>,
}

impl Context {
    pub(super) fn open(&self, point: Point, target: Target) {
        let Some(window) = self.window.borrow().clone() else {
            return;
        };
        let mut items = Vec::new();
        let bar = self.quick.preview(&self.bar);
        if let Target::Item { group, item } = target {
            if let Some(entry) = bar.sections.get(group).and_then(|section| {
                coconut_core::ordered_islands(&section.islands)
                    .get(item)
                    .copied()
            }) {
                if let Some(route) = bar_editor::item_settings_route(&entry.id) {
                    items.push(creamui_widgets::ContextMenuItem::new(
                        "item-settings",
                        "Configure widget…",
                        move || coconut_plugin_kit::open_settings(route),
                    ));
                }
            }
            if self.quick.is_active() {
                let quick = self.quick.clone();
                items.push(creamui_widgets::ContextMenuItem::new(
                    "remove-item",
                    "Remove item",
                    move || quick.apply(coconut_core::BarEdit::RemoveItem { group, item }),
                ));
            }
        }
        if let Target::Group(group) = target {
            if self.quick.is_active() {
                let quick = self.quick.clone();
                items.push(creamui_widgets::ContextMenuItem::new(
                    "add-item",
                    "Add item…",
                    move || quick.open_catalog(group),
                ));
                let quick = self.quick.clone();
                items.push(
                    creamui_widgets::ContextMenuItem::new(
                        "remove-group",
                        "Remove empty section",
                        move || quick.apply(coconut_core::BarEdit::RemoveGroup(group)),
                    )
                    .enabled(
                        bar.sections
                            .get(group)
                            .is_some_and(|section| section.islands.is_empty()),
                    ),
                );
            }
        }
        let route = self.kind.route().to_owned();
        items.push(creamui_widgets::ContextMenuItem::new(
            "settings",
            if self.kind == BarKind::Statusbar {
                "Bar settings…"
            } else {
                "Dock settings…"
            },
            move || coconut_plugin_kit::open_settings(route.clone()),
        ));
        let quick = self.quick.clone();
        let quick_window = window.clone();
        let bar = self.bar.clone();
        let kind = self.kind;
        let available = self.available.clone();
        items.insert(
            0,
            creamui_widgets::ContextMenuItem::new("quick-edit", "Quick Edit", move || {
                quick.open(quick_window.clone(), kind, bar.clone(), available.clone())
            }),
        );
        if self.quick.is_active() {
            let quick = self.quick.clone();
            let parent = window.clone();
            let kind = self.kind;
            items.insert(
                1,
                creamui_widgets::ContextMenuItem::new("save", "Save quick edits", move || {
                    quick.save(&parent, kind)
                }),
            );
            let quick = self.quick.clone();
            items.insert(
                2,
                creamui_widgets::ContextMenuItem::new(
                    "discard",
                    "Discard quick edits",
                    move || quick.discard(),
                ),
            );
        }
        let editor = self.editor.clone();
        let kind = self.kind;
        let bar = self.bar.clone();
        let undo_window = window.clone();
        if !self.quick.is_active() && self.editor.can_undo(self.kind, &self.bar) {
            items.push(creamui_widgets::ContextMenuItem::new(
                "undo",
                "Undo last edit",
                move || editor.undo(&undo_window, kind, &bar, Rc::new(|_| {})),
            ));
        }
        self.editor.open(window, point, items);
    }
}

pub fn wrap(widget: BoxedWidget, target: Target) -> BoxedWidget {
    let Some(context) = creamui_reactive::try_use_context::<Context>() else {
        return widget;
    };
    if context.quick.is_active() && !matches!(target, Target::Bar) {
        return widget;
    }
    Box::new(
        RawView::new(creamui_core::Style::new())
            .with_context_menu(move |point| context.open(point, target))
            .child(widget),
    )
}

pub fn root(widget: BoxedWidget, size: Size, context: &Context) -> BoxedWidget {
    let children = std::iter::once(wrap(widget, Target::Bar))
        .chain(context.editor.overlays(size))
        .collect();
    Box::new(
        Flex::column()
            .size(size.width, size.height)
            .with_children(children),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use coconut_core::{DockPosition, IslandEntry, SectionConfig};
    use coconut_plugin_kit::{Island, IslandRenderContext, SharedState};
    use creamui_core::{Renderer, Styled};
    use creamui_render::SceneRecorder;
    use creamui_theme::{Theme, ThemeProvider};
    use creamui_widgets::RawButton;
    use std::cell::Cell;

    struct Item(Rc<Cell<usize>>);
    impl Island for Item {
        fn id(&self) -> &'static str {
            "test"
        }
        fn build(&self, _: &IslandRenderContext<'_>) -> BoxedWidget {
            let clicks = self.0.clone();
            Box::new(
                RawButton::new(
                    creamui_core::Style::new().width(28.0).height(28.0),
                    move || clicks.set(clicks.get() + 1),
                )
                .background(creamui_theme::Color::rgb(20, 120, 220)),
            )
        }
    }

    #[test]
    fn contextual_wrappers_preserve_bar_geometry_and_primary_actions() {
        for position in [
            DockPosition::Top,
            DockPosition::Bottom,
            DockPosition::Left,
            DockPosition::Right,
        ] {
            creamui_reactive::with_context_scope(|| {
                let theme = Theme::dark();
                creamui_reactive::provide_context(ThemeProvider::new(theme));
                let size = if position.is_vertical() {
                    Size {
                        width: 44.0,
                        height: 300.0,
                    }
                } else {
                    Size {
                        width: 300.0,
                        height: 44.0,
                    }
                };
                let bar = DockConfig {
                    position,
                    sections: vec![SectionConfig {
                        islands: vec![IslandEntry::with_id("test"), IslandEntry::with_id("test")],
                        ..Default::default()
                    }],
                    ..Default::default()
                };
                let clicks = Rc::new(Cell::new(0));
                let item: Rc<dyn Island> = Rc::new(Item(clicks.clone()));
                let shared = SharedState::default();
                let panel: Rc<dyn Fn(&'static str, Point)> = Rc::new(|_, _| {});
                let widget = super::super::build_dock_with_lookup(
                    size,
                    &bar,
                    &|_| Some(item.clone()),
                    &shared,
                    &panel,
                );
                let mut recorder = SceneRecorder::new();
                recorder.begin(
                    size.width as u32,
                    size.height as u32,
                    1.0,
                    theme.colors.surface,
                    theme.colors,
                );
                let original = Renderer::new().render(widget, size, &mut recorder);
                let context = Context {
                    editor: BarEditor::default(),
                    window: Rc::new(RefCell::new(None)),
                    kind: BarKind::Statusbar,
                    bar: bar.clone(),
                    quick: super::super::quick::State::default(),
                    available: Rc::new(Vec::new()),
                };
                creamui_reactive::provide_context(context.clone());
                let widget = super::super::build_dock_with_lookup(
                    size,
                    &bar,
                    &|_| Some(item.clone()),
                    &shared,
                    &panel,
                );
                recorder.begin(
                    size.width as u32,
                    size.height as u32,
                    1.0,
                    theme.colors.surface,
                    theme.colors,
                );
                let edited =
                    Renderer::new().render(root(widget, size, &context), size, &mut recorder);
                let mut hits = 0;
                for y in (0..size.height as u32).step_by(4) {
                    for x in (0..size.width as u32).step_by(4) {
                        let point = Point {
                            x: x as f32,
                            y: y as f32,
                        };
                        assert_eq!(
                            original.hit_test(point).is_some(),
                            edited.hit_test(point).is_some(),
                            "primary hit geometry changed: {position:?} at {point:?}"
                        );
                        if let Some(click) = edited.hit_test(point) {
                            click();
                            hits += 1;
                        }
                        assert!(
                            edited.context_menu_at(point).is_some(),
                            "bar background has a menu"
                        );
                    }
                }
                assert!(hits > 0);
                assert_eq!(clicks.get(), hits);
            });
        }
    }
}
