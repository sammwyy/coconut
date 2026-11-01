use creamui_core::{BoxedWidget, FileDropTarget, Key, Painter, Point, Rect, Size, Style, Widget};
use creamui_render::{ContextMenuHost, PopupOptions, WindowHandle};
use creamui_widgets::{layout::Flex, ContextMenuItem};
use std::{
    cell::{Cell, RefCell},
    path::{Path, PathBuf},
    rc::{Rc, Weak},
};

type MenuBuilder = Rc<dyn Fn(Point) -> Vec<ContextMenuItem>>;

#[derive(Clone, Default)]
pub struct FileDropFilter {
    extensions: Vec<String>,
}

impl FileDropFilter {
    pub fn extensions(extensions: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            extensions: extensions
                .into_iter()
                .map(|ext| ext.into().trim_start_matches('.').to_ascii_lowercase())
                .collect(),
        }
    }
    pub fn accepts(&self, path: &Path) -> bool {
        self.extensions.is_empty()
            || path
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| {
                    self.extensions
                        .iter()
                        .any(|allowed| ext.eq_ignore_ascii_case(allowed))
                })
    }
}

#[derive(Clone, Default)]
pub struct InteractionHost(Rc<HostState>);

#[derive(Clone)]
pub struct WeakInteractionHost(Weak<HostState>);

impl WeakInteractionHost {
    pub fn upgrade(&self) -> Option<InteractionHost> {
        self.0.upgrade().map(InteractionHost)
    }
}

#[derive(Default)]
struct HostState {
    window: RefCell<Option<WindowHandle>>,
    menu: ContextMenuHost,
}

impl InteractionHost {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn downgrade(&self) -> WeakInteractionHost {
        WeakInteractionHost(Rc::downgrade(&self.0))
    }
    pub fn close_window(&self) {
        if let Some(window) = self.0.window.borrow().as_ref() {
            window.close();
        }
    }

    pub fn install(&self, window: WindowHandle) {
        *self.0.window.borrow_mut() = Some(window);
    }

    pub fn is_dragging(&self) -> bool {
        self.0
            .window
            .borrow()
            .as_ref()
            .is_some_and(WindowHandle::is_dragging)
    }

    pub fn menu_open(&self) -> bool {
        self.0.menu.is_open()
    }
    pub fn native_menu_open(&self) -> bool {
        self.0.menu.uses_native_popup()
    }
    pub fn parent_focus_lost(&self) {
        self.0.menu.parent_focus_lost();
    }
    pub fn handle_key(&self, key: &Key, pressed: bool) -> bool {
        pressed && self.0.menu.handle_key(key)
    }

    pub fn open_menu(&self, at: Point, items: Vec<ContextMenuItem>) {
        if let Some(window) = self.0.window.borrow().clone() {
            self.0.menu.open(PopupOptions::at_cursor(window, at), items);
        }
    }

    pub fn root(&self, child: BoxedWidget, size: Size) -> BoxedWidget {
        let mut root = Flex::column().size(size.width, size.height).child(child);
        if let Some(menu) = self.0.menu.overlay(size) {
            root = root.child(menu);
        }
        Box::new(root)
    }

    pub fn start_file_drag(&self, paths: &[PathBuf]) -> Result<(), String> {
        let window = self
            .0
            .window
            .borrow()
            .clone()
            .ok_or_else(|| "No interaction window".to_owned())?;
        window.start_drag_with_data(&file_drag_data(paths)?, None)
    }
}

pub fn file_drag_data(paths: &[PathBuf]) -> Result<Vec<(String, Vec<u8>)>, String> {
    let payload = paths
        .iter()
        .map(|path| {
            url::Url::from_file_path(path)
                .map(|url| format!("{url}\r\n"))
                .map_err(|_| format!("Invalid drag path: {}", path.display()))
        })
        .collect::<Result<String, _>>()?;
    Ok(vec![("text/uri-list".into(), payload.into_bytes())])
}

pub struct Interactive {
    style: Style,
    child: Option<BoxedWidget>,
    host: InteractionHost,
    menu: Option<MenuBuilder>,
    drop: Option<FileDropTarget>,
    drag_files: Vec<PathBuf>,
    drag_origin: Rc<Cell<Point>>,
    dragging: Rc<Cell<bool>>,
    click: Option<Rc<dyn Fn(Point)>>,
    bounds: Rc<Cell<Rect>>,
}

impl Interactive {
    pub fn new(child: BoxedWidget) -> Self {
        Self::with_host(
            child,
            creamui_reactive::try_use_context::<InteractionHost>().unwrap_or_default(),
        )
    }

    pub fn with_host(child: BoxedWidget, host: InteractionHost) -> Self {
        Self {
            style: Style::new().flex_shrink(0.0),
            child: Some(child),
            host,
            menu: None,
            drop: None,
            drag_files: Vec::new(),
            drag_origin: Default::default(),
            dragging: Default::default(),
            click: None,
            bounds: Default::default(),
        }
    }

    pub fn with_style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    pub fn context_menu(mut self, menu: impl Fn(Point) -> Vec<ContextMenuItem> + 'static) -> Self {
        self.menu = Some(Rc::new(menu));
        self
    }

    pub fn on_click(mut self, click: impl Fn(Point) + 'static) -> Self {
        self.click = Some(Rc::new(click));
        self
    }

    pub fn on_drop_files(
        mut self,
        filter: FileDropFilter,
        handler: impl Fn(Vec<PathBuf>) + 'static,
    ) -> Self {
        let target = FileDropTarget {
            accepts: Rc::new(move |path| filter.accepts(path)),
            on_drop: Rc::new(handler),
        };
        self.drop = Some(target);
        self
    }

    pub fn drag_files(mut self, paths: Vec<PathBuf>) -> Self {
        self.drag_files = paths;
        self
    }
}

impl Widget for Interactive {
    fn style(&self) -> Style {
        self.style.clone()
    }
    fn children(&mut self) -> Vec<BoxedWidget> {
        self.child.take().into_iter().collect()
    }
    fn paint(&self, _: &mut dyn Painter, _: Rect) {}
    fn reports_layout(&self) -> bool {
        true
    }
    fn on_layout(&self, rect: Rect) -> bool {
        self.bounds.set(rect);
        false
    }
    fn file_drop_target(&self) -> Option<FileDropTarget> {
        self.drop.clone()
    }
    fn focusable(&self) -> bool {
        self.click.is_some()
    }
    fn on_click(&self) -> Option<Rc<dyn Fn()>> {
        let click = self.click.clone()?;
        let bounds = self.bounds.clone();
        Some(Rc::new(move || {
            let rect = bounds.get();
            click(Point {
                x: rect.x + rect.width / 2.0,
                y: rect.y + rect.height / 2.0,
            });
        }))
    }
    fn on_click_at(&self) -> Option<Rc<dyn Fn(Point)>> {
        self.click.clone()
    }
    fn on_key(&self) -> Option<Rc<dyn Fn(creamui_core::KeyInput)>> {
        let click = Widget::on_click(self)?;
        Some(Rc::new(move |input| {
            if !input.modifiers.ctrl
                && !input.modifiers.alt
                && matches!(input.key, Key::Enter | Key::Char(' '))
            {
                click();
            }
        }))
    }
    fn on_context_menu(&self) -> Option<Rc<dyn Fn(Point)>> {
        let menu = self.menu.clone()?;
        let host = self.host.clone();
        Some(Rc::new(move |at| host.open_menu(at, menu(at))))
    }
    fn on_drag_start(&self) -> Option<Rc<dyn Fn(Point, Rect)>> {
        if self.drag_files.is_empty() {
            return None;
        }
        let origin = self.drag_origin.clone();
        let dragging = self.dragging.clone();
        Some(Rc::new(move |point, _| {
            origin.set(point);
            dragging.set(false);
        }))
    }
    fn on_drag(&self) -> Option<Rc<dyn Fn(Point, Rect)>> {
        if self.drag_files.is_empty() {
            return None;
        }
        let host = self.host.clone();
        let paths = self.drag_files.clone();
        let origin = self.drag_origin.clone();
        let dragging = self.dragging.clone();
        Some(Rc::new(move |point, _| {
            let start = origin.get();
            if !dragging.get() && (point.x - start.x).hypot(point.y - start.y) >= 6.0 {
                dragging.set(true);
                if let Err(error) = host.start_file_drag(&paths) {
                    eprintln!("plugin-kit: cannot drag files: {error}");
                }
            }
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use creamui_core::Renderer;
    use creamui_render::SceneRecorder;
    use creamui_theme::Color;

    #[test]
    fn widgets_own_runtime_menus_and_only_receive_files_within_their_bounds() {
        let host = InteractionHost::new();
        let delivered = Rc::new(RefCell::new(Vec::new()));
        let menus = Rc::new(Cell::new(0));
        let clicks = Rc::new(Cell::new(0));
        let drop_result = delivered.clone();
        let menu_count = menus.clone();
        let click_count = clicks.clone();
        let child = Interactive::with_host(Box::new(Flex::row().size(80.0, 40.0)), host.clone())
            .on_click(move |_| click_count.set(click_count.get() + 1))
            .context_menu(move |_| {
                menu_count.set(menu_count.get() + 1);
                vec![ContextMenuItem::new(
                    "custom",
                    format!("Action {}", menu_count.get()),
                    || {},
                )]
            })
            .on_drop_files(FileDropFilter::extensions([".desktop"]), move |paths| {
                drop_result.borrow_mut().extend(paths)
            });
        let mut recorder = SceneRecorder::new();
        let size = Size {
            width: 240.0,
            height: 80.0,
        };
        recorder.begin(240, 80, 1.0, Color::rgb(0, 0, 0), Default::default());
        let mut renderer = Renderer::new();
        let scene = renderer.render(
            Box::new(
                Flex::row()
                    .size(240.0, 80.0)
                    .child(Box::new(Flex::row().size(40.0, 40.0)))
                    .child(Box::new(child)),
            ),
            size,
            &mut recorder,
        );
        let point = Point { x: 65.0, y: 20.0 };
        scene.context_menu_at(point).unwrap()(point);
        scene.context_menu_at(point).unwrap()(point);
        assert_eq!(menus.get(), 2);
        assert_eq!(clicks.get(), 0);
        assert!(scene
            .file_drop_target_at(Point { x: 10.0, y: 20.0 })
            .is_none());
        assert!(!scene
            .file_drop_target_at(point)
            .unwrap()
            .dispatch(vec!["/tmp/photo.png".into()]));
        assert!(scene.file_drop_target_at(point).unwrap().dispatch(vec![
            "/tmp/App.desktop".into(),
            "/tmp/Other.DESKTOP".into(),
            "/tmp/photo.png".into()
        ]));
        assert_eq!(
            *delivered.borrow(),
            vec![
                PathBuf::from("/tmp/App.desktop"),
                PathBuf::from("/tmp/Other.DESKTOP")
            ]
        );
        scene.hit_test(point).unwrap()();
        assert_eq!(clicks.get(), 1);
        let empty = renderer.render(Box::new(Flex::row().size(240.0, 80.0)), size, &mut recorder);
        assert!(empty.file_drop_target_at(point).is_none());
    }

    #[test]
    fn file_drops_respect_visible_clips_and_modal_barriers() {
        struct Barrier {
            modal: bool,
        }
        impl Widget for Barrier {
            fn style(&self) -> Style {
                Style::new()
                    .width(40.0)
                    .height(40.0)
                    .position(creamui_core::layout::Position::Absolute)
            }
            fn paint(&self, _: &mut dyn Painter, _: Rect) {}
            fn blocks_pointer(&self) -> bool {
                true
            }
            fn is_modal(&self) -> bool {
                self.modal
            }
        }
        let size = Size {
            width: 80.0,
            height: 80.0,
        };
        let mut recorder = SceneRecorder::new();
        let mut renderer = Renderer::new();
        recorder.begin(80, 80, 1.0, Color::rgb(0, 0, 0), Default::default());
        let target = || {
            Box::new(
                Interactive::new(Box::new(Flex::row().size(80.0, 80.0)))
                    .on_drop_files(FileDropFilter::default(), |_| {}),
            ) as BoxedWidget
        };
        let clipped = renderer.render(
            Box::new(
                creamui_widgets::RawView::new(Style::new().width(80.0).height(40.0))
                    .child(target()),
            ),
            size,
            &mut recorder,
        );
        assert!(clipped
            .file_drop_target_at(Point { x: 20.0, y: 20.0 })
            .is_some());
        assert!(clipped
            .file_drop_target_at(Point { x: 20.0, y: 60.0 })
            .is_none());
        for modal in [false, true] {
            let scene = renderer.render(
                Box::new(
                    Flex::column()
                        .size(80.0, 80.0)
                        .child(target())
                        .child(Box::new(Barrier { modal })),
                ),
                size,
                &mut recorder,
            );
            assert!(scene
                .file_drop_target_at(Point { x: 20.0, y: 20.0 })
                .is_none());
            assert_eq!(
                scene
                    .file_drop_target_at(Point { x: 60.0, y: 20.0 })
                    .is_some(),
                !modal
            );
        }
    }

    #[test]
    fn drag_sources_preserve_clicks_and_have_a_motion_threshold() {
        let host = InteractionHost::new();
        let clicked = Rc::new(Cell::new(0));
        let click = clicked.clone();
        let widget = Interactive::with_host(Box::new(Flex::row().size(40.0, 40.0)), host)
            .on_click(move |_| click.set(click.get() + 1))
            .drag_files(vec!["/tmp/App.desktop".into()]);
        let rect = Rect {
            x: 0.0,
            y: 0.0,
            width: 40.0,
            height: 40.0,
        };
        widget.on_drag_start().unwrap()(Point { x: 20.0, y: 20.0 }, rect);
        widget.on_drag().unwrap()(Point { x: 22.0, y: 21.0 }, rect);
        assert!(!widget.dragging.get());
        Widget::on_click(&widget).unwrap()();
        assert_eq!(clicked.get(), 1);
        widget.on_drag().unwrap()(Point { x: 30.0, y: 20.0 }, rect);
        assert!(widget.dragging.get());
    }
}
