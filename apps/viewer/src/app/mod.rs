mod canvas;
mod commands;
#[cfg(test)]
mod tests;
mod views;

use crate::{
    asset::{self, Asset},
    view::{Adjustments, View, Zoom},
};
use creamui_core::{Point, Rect, Size};
use creamui_reactive::{batch, Signal};
use creamui_render::{AppHandle, WindowHandle};
use creamui_widgets::{CommandPaletteController, ScrollController};
use std::{
    cell::{Cell, RefCell},
    path::PathBuf,
    rc::Rc,
    sync::Arc,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Panel {
    #[default]
    None,
    Adjustments,
    Information,
}
#[derive(Clone, Copy)]
pub enum Action {
    Open,
    Close,
    Previous,
    Next,
    First,
    Last,
    Fit,
    ActualSize,
    ZoomIn,
    ZoomOut,
    Rotate(bool),
    Panel(Panel),
    Brightness(f32),
    Contrast(f32),
    Saturation(f32),
    Grayscale,
    Invert,
    ResetAdjustments,
    ResetView,
    Commands,
    Dismiss,
    Retry,
}
struct Model {
    asset: Option<Arc<Asset>>,
    paths: Rc<Vec<PathBuf>>,
    index: usize,
    view: View,
    error: Option<String>,
}
impl Default for Model {
    fn default() -> Self {
        Self {
            asset: None,
            paths: Rc::new(vec![]),
            index: 0,
            view: View::default(),
            error: None,
        }
    }
}
#[derive(Clone)]
struct Request {
    generation: u64,
    path: PathBuf,
    discover: bool,
}
struct Loaded {
    asset: Asset,
    playlist: Option<Vec<PathBuf>>,
}
#[derive(Clone, Copy)]
enum Drag {
    Image { origin: Point, pan: Point },
    Map,
}
#[derive(Clone)]
pub struct State {
    app: Option<AppHandle>,
    pub window: Rc<RefCell<Option<WindowHandle>>>,
    model: Rc<RefCell<Model>>,
    revision: Signal<u64>,
    panel: Signal<Panel>,
    palette_open: Signal<bool>,
    palette: CommandPaletteController,
    panel_scroll: ScrollController,
    loading: Signal<bool>,
    dialog_open: Signal<bool>,
    generation: Rc<Cell<u64>>,
    busy: Rc<Cell<bool>>,
    pending: Rc<RefCell<Option<Request>>>,
    canvas: Rc<Cell<Rect>>,
    pointer: Rc<Cell<Point>>,
    drag: Rc<Cell<Option<Drag>>>,
}
impl State {
    pub fn new(app: AppHandle) -> Self {
        Self::base(Some(app))
    }
    fn base(app: Option<AppHandle>) -> Self {
        Self {
            app,
            window: Rc::new(RefCell::new(None)),
            model: Rc::new(RefCell::new(Model::default())),
            revision: Signal::new(0),
            panel: Signal::new(Panel::None),
            palette_open: Signal::new(false),
            palette: CommandPaletteController::new(),
            panel_scroll: ScrollController::default(),
            loading: Signal::new(false),
            dialog_open: Signal::new(false),
            generation: Rc::new(Cell::new(0)),
            busy: Rc::new(Cell::new(false)),
            pending: Rc::new(RefCell::new(None)),
            canvas: Rc::new(Cell::new(Rect::default())),
            pointer: Rc::new(Cell::new(Point::default())),
            drag: Rc::new(Cell::new(None)),
        }
    }
    fn refresh(&self) {
        self.revision.update(|value| *value = value.wrapping_add(1));
    }
    fn fail(&self, error: String) {
        eprintln!("cviewer: {error}");
        self.model.borrow_mut().error = Some(error);
        self.refresh();
    }
    fn update_title(&self) {
        let title = self.model.borrow().asset.as_ref().map_or_else(
            || "Image Viewer".into(),
            |asset| {
                format!(
                    "{} — Image Viewer",
                    asset.path.file_name().unwrap_or_default().to_string_lossy()
                )
            },
        );
        if let Some(window) = self.window.borrow().as_ref() {
            window.set_title(&title);
        }
    }
    pub fn open_paths(&self, paths: Vec<PathBuf>) {
        if paths.is_empty() {
            return;
        }
        if paths.len() > asset::MAX_DIRECTORY_IMAGES {
            self.fail("Open at most 10000 images at once".into());
            return;
        }
        batch(|| {
            self.model.borrow_mut().paths = Rc::new(paths);
            self.model.borrow_mut().index = 0;
            let discover = self.model.borrow().paths.len() == 1;
            self.request_current(discover);
        });
    }
    fn request_current(&self, discover: bool) {
        let path = {
            let model = self.model.borrow();
            model.paths.get(model.index).cloned()
        };
        let Some(path) = path else {
            return;
        };
        self.generation.set(self.generation.get().wrapping_add(1));
        let generation = self.generation.get();
        {
            let mut model = self.model.borrow_mut();
            model.asset = None;
            model.error = None;
            model.view = View::default();
        }
        *self.pending.borrow_mut() = Some(Request {
            generation,
            path,
            discover,
        });
        self.loading.set(true);
        self.update_title();
        self.drag.set(None);
        self.refresh();
        self.drive();
    }
    fn drive(&self) {
        if self.busy.get() {
            return;
        }
        let Some(request) = self.pending.borrow_mut().take() else {
            return;
        };
        let Some(app) = &self.app else {
            self.loading.set(false);
            self.fail("Image loading requires a running application".into());
            return;
        };
        self.busy.set(true);
        let done = self.clone();
        let generation = request.generation;
        app.spawn_background(
            move || {
                let path = asset::absolute(request.path)?;
                let mut image = asset::load(&path)?;
                let playlist = if request.discover {
                    match asset::neighbors(&path) {
                        Ok(paths) => Some(paths),
                        Err(error) => {
                            eprintln!("cviewer: {error}");
                            image.metadata.warnings.push(error);
                            Some(vec![path])
                        }
                    }
                } else {
                    None
                };
                Ok(Loaded {
                    asset: image,
                    playlist,
                })
            },
            move |result| {
                done.busy.set(false);
                done.accept(generation, result);
                done.drive();
            },
        );
    }
    fn accept(&self, generation: u64, result: Result<Loaded, String>) {
        if generation != self.generation.get() {
            return;
        }
        batch(|| {
            self.loading.set(false);
            match result {
                Ok(loaded) => {
                    let path = loaded.asset.path.clone();
                    {
                        let mut model = self.model.borrow_mut();
                        if let Some(paths) = loaded.playlist {
                            model.index =
                                paths.iter().position(|value| value == &path).unwrap_or(0);
                            model.paths = Rc::new(paths);
                        }
                        model.asset = Some(Arc::new(loaded.asset));
                        model.error = None;
                        model.view = View::default();
                    }
                    self.panel_scroll.set(0.0);
                }
                Err(error) => {
                    eprintln!("cviewer: {error}");
                    self.model.borrow_mut().error = Some(error);
                }
            }
            self.update_title();
            self.refresh();
        });
    }
    fn choose(&self) {
        if self.dialog_open.peek() {
            return;
        }
        let Some(app) = &self.app else {
            self.fail("File dialogs require a running application".into());
            return;
        };
        self.dialog_open.set(true);
        let state = self.clone();
        app.spawn_background(
            || {
                rfd::FileDialog::new()
                    .set_title("Open images")
                    .add_filter("Images", asset::EXTENSIONS)
                    .pick_files()
            },
            move |paths| {
                state.dialog_open.set(false);
                if let Some(paths) = paths {
                    state.open_paths(paths);
                }
            },
        );
    }
    fn viewport(&self) -> Size {
        let rect = self.canvas.get();
        Size {
            width: rect.width.max(1.0),
            height: rect.height.max(1.0),
        }
    }
    fn edit_view(&self, edit: impl FnOnce(&mut View, Size, Size)) {
        let viewport = self.viewport();
        {
            let mut model = self.model.borrow_mut();
            let Some(asset) = model.asset.as_ref() else {
                return;
            };
            let size = asset.size();
            edit(&mut model.view, viewport, size);
        }
        self.refresh();
    }
    pub fn pointer_moved(&self, point: Point) {
        self.pointer.set(point);
    }
    fn zoom(&self, factor: f32, anchor: Option<Point>) {
        let rect = self.canvas.get();
        let anchor = anchor.unwrap_or(Point {
            x: rect.width / 2.0,
            y: rect.height / 2.0,
        });
        self.edit_view(|view, viewport, image| view.zoom_at(factor, anchor, viewport, image));
    }
    fn wheel(&self, delta: f32) {
        let rect = self.canvas.get();
        let pointer = self.pointer.get();
        let anchor = if rect.contains(pointer) {
            Point {
                x: pointer.x - rect.x,
                y: pointer.y - rect.y,
            }
        } else {
            Point {
                x: rect.width / 2.0,
                y: rect.height / 2.0,
            }
        };
        self.zoom((-delta.clamp(-500.0, 500.0) / 320.0).exp(), Some(anchor));
    }
    fn drag_start(&self, point: Point) {
        let viewport = self.viewport();
        let model = self.model.borrow();
        let Some(asset) = model.asset.as_ref() else {
            return;
        };
        let view = model.view;
        let image = asset.size();
        let minimap = view.minimap(viewport, image);
        drop(model);
        if minimap.is_some_and(|(map, _)| map.contains(point)) {
            self.drag.set(Some(Drag::Map));
            self.edit_view(|view, viewport, image| view.navigate_map(point, viewport, image));
        } else {
            self.drag.set(Some(Drag::Image {
                origin: point,
                pan: view.layout(viewport, image).pan,
            }));
        }
    }
    fn drag_to(&self, point: Point) {
        match self.drag.get() {
            Some(Drag::Map) => {
                self.edit_view(|view, viewport, image| view.navigate_map(point, viewport, image))
            }
            Some(Drag::Image { origin, pan }) => self.edit_view(|view, viewport, image| {
                view.set_pan(
                    Point {
                        x: pan.x + point.x - origin.x,
                        y: pan.y + point.y - origin.y,
                    },
                    viewport,
                    image,
                )
            }),
            None => {}
        }
    }
    fn navigate(&self, index: usize) {
        let valid = {
            let mut model = self.model.borrow_mut();
            if index >= model.paths.len() {
                false
            } else {
                model.index = index;
                true
            }
        };
        if valid {
            self.request_current(false);
        }
    }
    pub fn run(&self, action: Action) {
        batch(|| {
            if !matches!(action, Action::Commands | Action::Dismiss) {
                self.palette_open.set(false);
            }
            match action {
                Action::Open => self.choose(),
                Action::Retry => {
                    let discover = self.model.borrow().paths.len() == 1;
                    self.request_current(discover);
                }
                Action::Close => {
                    self.generation.set(self.generation.get().wrapping_add(1));
                    self.pending.borrow_mut().take();
                    *self.model.borrow_mut() = Model::default();
                    self.loading.set(false);
                    self.panel.set(Panel::None);
                    self.update_title();
                }
                Action::Previous => {
                    let index = self.model.borrow().index;
                    if index > 0 {
                        self.navigate(index - 1);
                    }
                }
                Action::Next => {
                    let index = self.model.borrow().index;
                    self.navigate(index + 1);
                }
                Action::First => self.navigate(0),
                Action::Last => {
                    let length = self.model.borrow().paths.len();
                    if length > 0 {
                        self.navigate(length - 1);
                    }
                }
                Action::Fit => self.edit_view(|view, _, _| {
                    view.zoom = Zoom::Fit;
                    view.pan = Point::default();
                }),
                Action::ActualSize => self.edit_view(|view, viewport, image| {
                    view.zoom = Zoom::Scale(1.0);
                    view.pan = view.layout(viewport, image).pan;
                }),
                Action::ZoomIn => self.zoom(1.25, None),
                Action::ZoomOut => self.zoom(0.8, None),
                Action::Rotate(clockwise) => self.edit_view(|view, _, _| view.rotate(clockwise)),
                Action::Brightness(value) => {
                    self.edit_view(|view, _, _| view.adjustments.brightness = value.clamp(0.0, 2.0))
                }
                Action::Contrast(value) => {
                    self.edit_view(|view, _, _| view.adjustments.contrast = value.clamp(0.0, 2.0))
                }
                Action::Saturation(value) => self.edit_view(|view, _, _| {
                    view.adjustments.saturation = value.clamp(0.0, 2.0);
                    view.adjustments.grayscale = false;
                }),
                Action::Grayscale => self.edit_view(|view, _, _| {
                    view.adjustments.grayscale = !view.adjustments.grayscale
                }),
                Action::Invert => {
                    self.edit_view(|view, _, _| view.adjustments.invert = !view.adjustments.invert)
                }
                Action::ResetAdjustments => {
                    self.edit_view(|view, _, _| view.adjustments = Adjustments::default())
                }
                Action::ResetView => self.edit_view(|view, _, _| *view = View::default()),
                Action::Panel(panel) => {
                    self.panel.set(if self.panel.peek() == panel {
                        Panel::None
                    } else {
                        panel
                    });
                    self.panel_scroll.set(0.0);
                }
                Action::Commands => {
                    self.palette.reset();
                    self.palette_open.set(true);
                }
                Action::Dismiss => {
                    if self.palette_open.peek() {
                        self.palette_open.set(false);
                    } else if self.model.borrow().asset.is_some()
                        && self.model.borrow().error.is_some()
                    {
                        self.model.borrow_mut().error = None;
                    } else {
                        self.panel.set(Panel::None);
                        self.model.borrow_mut().error = None;
                    }
                }
            }
            self.refresh();
        });
        if self.palette_open.peek() {
            if let Some(window) = self.window.borrow().as_ref() {
                window.focus_text_input();
            }
        }
    }
}
