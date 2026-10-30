mod canvas;
mod hud;
mod visual_tests;

use crate::image::{Annotations, Dab, Shot};
use crate::paths;
use creamui_core::{Point, Rect, RgbaImage, Size};
use creamui_reactive::Signal;
use creamui_render::{
    platform::{Key, KeyEvent, Modifiers, MouseButton},
    AppHandle, WindowHandle,
};
use hud::{HudId, Tool};
use std::{
    cell::{Cell, Ref, RefCell},
    rc::Rc,
};

#[derive(Clone, Copy, PartialEq, Debug)]
enum Press {
    NewSelection { anchor: Point },
    Move { offset: Point },
    Resize { handle: ResizeHandle, start: Rect },
    Draw { last: Point },
    Hud(HudId),
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum ResizeHandle {
    Nw,
    N,
    Ne,
    E,
    Se,
    S,
    Sw,
    W,
}

const HANDLES: [ResizeHandle; 8] = [
    ResizeHandle::Nw,
    ResizeHandle::N,
    ResizeHandle::Ne,
    ResizeHandle::E,
    ResizeHandle::Se,
    ResizeHandle::S,
    ResizeHandle::Sw,
    ResizeHandle::W,
];

impl ResizeHandle {
    fn anchor(self, rect: Rect) -> Point {
        let (cx, cy) = (rect.x + rect.width / 2.0, rect.y + rect.height / 2.0);
        match self {
            Self::Nw => Point {
                x: rect.x,
                y: rect.y,
            },
            Self::N => Point { x: cx, y: rect.y },
            Self::Ne => Point {
                x: rect.x + rect.width,
                y: rect.y,
            },
            Self::E => Point {
                x: rect.x + rect.width,
                y: cy,
            },
            Self::Se => Point {
                x: rect.x + rect.width,
                y: rect.y + rect.height,
            },
            Self::S => Point {
                x: cx,
                y: rect.y + rect.height,
            },
            Self::Sw => Point {
                x: rect.x,
                y: rect.y + rect.height,
            },
            Self::W => Point { x: rect.x, y: cy },
        }
    }

    fn apply(self, start: Rect, point: Point, bounds: Size) -> Rect {
        let (mut x0, mut y0) = (start.x, start.y);
        let (mut x1, mut y1) = (start.x + start.width, start.y + start.height);
        match self {
            Self::Nw | Self::N | Self::Ne => y0 = point.y.min(y1),
            Self::Sw | Self::S | Self::Se => y1 = point.y.max(y0),
            _ => {}
        }
        match self {
            Self::Nw | Self::W | Self::Sw => x0 = point.x.min(x1),
            Self::Ne | Self::E | Self::Se => x1 = point.x.max(x0),
            _ => {}
        }
        Rect {
            x: x0.clamp(0.0, bounds.width),
            y: y0.clamp(0.0, bounds.height),
            width: (x1 - x0).min(bounds.width),
            height: (y1 - y0).min(bounds.height),
        }
    }
}

pub struct Model {
    pub shot: Shot,
    pub image: RgbaImage,
    pub annotations: Annotations,
    annotation_image: RefCell<Option<RgbaImage>>,
    pub blurred: Option<Shot>,
    pub selection: Option<Rect>,
    pub tool: Tool,
    pub color: usize,
    pub radius: usize,
    pub status: Option<String>,
    pub icons: hud::HudIcons,
}

impl Model {
    /// The annotation layer as a paintable image, regenerated only after a
    /// stroke dirtied the pixel buffer.
    pub fn annotation_image(&self) -> Option<RgbaImage> {
        if self.annotations.is_empty() {
            return None;
        }
        let mut cached = self.annotation_image.borrow_mut();
        if cached.is_none() {
            *cached = RgbaImage::new(
                self.annotations.width,
                self.annotations.height,
                self.annotations.pixels.clone(),
            );
        }
        cached.clone()
    }

    fn invalidate_annotations(&self) {
        *self.annotation_image.borrow_mut() = None;
    }
}

#[derive(Clone)]
pub struct State {
    app: Option<AppHandle>,
    pub window: Rc<RefCell<Option<WindowHandle>>>,
    model: Rc<RefCell<Model>>,
    revision: Signal<u64>,
    pointer: Rc<Cell<Point>>,
    press: Rc<Cell<Option<Press>>>,
    viewport: Rc<Cell<Rect>>,
    hovered: Rc<Cell<Option<HudId>>>,
    finishing: Rc<Cell<bool>>,
}

impl State {
    pub fn new(app: AppHandle, shot: Shot) -> Self {
        Self::base(Some(app), shot)
    }

    fn base(app: Option<AppHandle>, shot: Shot) -> Self {
        let image = RgbaImage::new(shot.width, shot.height, shot.pixels.clone())
            .expect("the capture validates its own buffer");
        let annotations = Annotations::new(shot.width, shot.height);
        Self {
            app,
            window: Rc::new(RefCell::new(None)),
            model: Rc::new(RefCell::new(Model {
                shot,
                image,
                annotations,
                annotation_image: RefCell::new(None),
                blurred: None,
                selection: None,
                tool: Tool::Select,
                color: 0,
                radius: 1,
                status: None,
                icons: hud::HudIcons::load(),
            })),
            revision: Signal::new(0),
            pointer: Rc::new(Cell::new(Point::default())),
            press: Rc::new(Cell::new(None)),
            viewport: Rc::new(Cell::new(Rect::default())),
            hovered: Rc::new(Cell::new(None)),
            finishing: Rc::new(Cell::new(false)),
        }
    }

    fn refresh(&self) {
        self.revision.update(|value| *value = value.wrapping_add(1));
    }

    pub(super) fn model(&self) -> Ref<'_, Model> {
        self.model.borrow()
    }

    #[cfg(test)]
    fn model_mut(&self) -> std::cell::RefMut<'_, Model> {
        self.model.borrow_mut()
    }

    /// Image-space size of the capture.
    fn bounds(&self) -> Size {
        let model = self.model.borrow();
        Size {
            width: model.shot.width as f32,
            height: model.shot.height as f32,
        }
    }

    /// Fit-to-view transform: `(scale, origin)` mapping image to widget
    /// coordinates.
    fn transform(&self) -> (f32, Point) {
        transform(self.viewport.get(), self.bounds())
    }

    fn to_image(&self, point: Point) -> Point {
        let (scale, origin) = self.transform();
        Point {
            x: (point.x - origin.x) / scale,
            y: (point.y - origin.y) / scale,
        }
    }

    fn image_rect(&self) -> Rect {
        let (scale, origin) = self.transform();
        let bounds = self.bounds();
        Rect {
            x: origin.x,
            y: origin.y,
            width: bounds.width * scale,
            height: bounds.height * scale,
        }
    }

    fn selection_view(&self) -> Option<Rect> {
        let (scale, origin) = self.transform();
        self.model.borrow().selection.map(|rect| Rect {
            x: origin.x + rect.x * scale,
            y: origin.y + rect.y * scale,
            width: rect.width * scale,
            height: rect.height * scale,
        })
    }

    fn hud(&self) -> Vec<(HudId, Rect)> {
        hud::layout(self.selection_view(), {
            let viewport = self.viewport.get();
            Size {
                width: viewport.width,
                height: viewport.height,
            }
        })
        .1
    }

    /// Recomputes which HUD item (if any) sits under the pointer and asks for
    /// a repaint when it changed.
    fn update_hover(&self) {
        let point = self.pointer.get();
        let hover = hud::hit(&self.hud(), point);
        if hover != self.hovered.get() {
            self.hovered.set(hover);
            self.refresh();
        }
    }

    pub fn pointer_moved(&self, point: Point) {
        self.pointer.set(point);
        if let Some(Press::Move { .. }) = self.press.get() {
            self.move_to(point);
        }
        self.update_hover();
    }

    /// Right button: press starts moving the selection, release ends it.
    /// Consumed so widget dispatch never sees secondary buttons.
    pub fn pointer_button(&self, button: MouseButton, pressed: bool, point: Point) -> bool {
        self.pointer.set(point);
        match (button, pressed) {
            (MouseButton::Right, true) => {
                let image = self.to_image(point);
                if let Some(selection) = self.model.borrow().selection {
                    self.press.set(Some(Press::Move {
                        offset: Point {
                            x: image.x - selection.x,
                            y: image.y - selection.y,
                        },
                    }));
                }
                true
            }
            (MouseButton::Right, false) => {
                if matches!(self.press.get(), Some(Press::Move { .. })) {
                    self.press.set(None);
                }
                true
            }
            _ => false,
        }
    }

    fn move_to(&self, point: Point) {
        let Some(Press::Move { offset }) = self.press.get() else {
            return;
        };
        let image = self.to_image(point);
        let bounds = self.bounds();
        let mut model = self.model.borrow_mut();
        if let Some(selection) = model.selection.as_mut() {
            selection.x =
                (image.x - offset.x).clamp(0.0, (bounds.width - selection.width).max(0.0));
            selection.y =
                (image.y - offset.y).clamp(0.0, (bounds.height - selection.height).max(0.0));
        }
        drop(model);
        self.refresh();
    }

    /// Left-button press routed from the canvas widget, in window
    /// coordinates.
    pub(super) fn drag_start_at(&self, point: Point) {
        self.pointer.set(point);
        if let Some(id) = hud::hit(&self.hud(), point) {
            self.press.set(Some(Press::Hud(id)));
            self.activate(id);
            return;
        }
        let image = self.to_image(point);
        let model = self.model.borrow();
        let tool = model.tool;
        let selection = model.selection;
        drop(model);
        match tool {
            Tool::Select => {
                let handle = selection.and_then(|rect| self.handle_at(point, rect));
                if let Some(handle) = handle {
                    self.press.set(Some(Press::Resize {
                        handle,
                        start: selection.unwrap_or_default(),
                    }));
                    return;
                }
                let view = self.selection_view();
                if let (Some(selection), Some(view)) = (selection, view) {
                    if view.contains(point) {
                        self.press.set(Some(Press::Move {
                            offset: Point {
                                x: image.x - selection.x,
                                y: image.y - selection.y,
                            },
                        }));
                        return;
                    }
                }
                self.press.set(Some(Press::NewSelection { anchor: image }));
                self.model.borrow_mut().selection = Some(Rect {
                    x: image.x,
                    y: image.y,
                    width: 0.0,
                    height: 0.0,
                });
                self.refresh();
            }
            _ => {
                self.press.set(Some(Press::Draw { last: image }));
                self.paint_stroke(image, image);
            }
        }
    }

    fn handle_at(&self, point: Point, selection: Rect) -> Option<ResizeHandle> {
        let (scale, origin) = self.transform();
        let view = Rect {
            x: origin.x + selection.x * scale,
            y: origin.y + selection.y * scale,
            width: selection.width * scale,
            height: selection.height * scale,
        };
        HANDLES.into_iter().find(|handle| {
            let anchor = handle.anchor(view);
            (point.x - anchor.x).abs() <= 6.0 && (point.y - anchor.y).abs() <= 6.0
        })
    }

    pub(super) fn drag_to_at(&self, point: Point) {
        self.pointer.set(point);
        let image = self.to_image(point);
        match self.press.get() {
            Some(Press::NewSelection { anchor }) => {
                let mut model = self.model.borrow_mut();
                let bounds = Size {
                    width: model.shot.width as f32,
                    height: model.shot.height as f32,
                };
                let clamp = |p: Point| Point {
                    x: p.x.clamp(0.0, bounds.width),
                    y: p.y.clamp(0.0, bounds.height),
                };
                let (a, b) = (clamp(anchor), clamp(image));
                model.selection = Some(Rect {
                    x: a.x.min(b.x),
                    y: a.y.min(b.y),
                    width: (a.x - b.x).abs(),
                    height: (a.y - b.y).abs(),
                });
                drop(model);
                self.refresh();
            }
            Some(Press::Resize { handle, start }) => {
                let mut model = self.model.borrow_mut();
                model.selection = Some(handle.apply(
                    start,
                    image,
                    Size {
                        width: model.shot.width as f32,
                        height: model.shot.height as f32,
                    },
                ));
                drop(model);
                self.refresh();
            }
            Some(Press::Move { .. }) => self.move_to(point),
            Some(Press::Draw { last }) => {
                self.paint_stroke(last, image);
                self.press.set(Some(Press::Draw { last: image }));
            }
            _ => {}
        }
    }

    pub(super) fn drag_finished(&self) {
        if let Some(Press::NewSelection { .. }) = self.press.get() {
            let mut model = self.model.borrow_mut();
            if model
                .selection
                .is_some_and(|rect| rect.width < 3.0 || rect.height < 3.0)
            {
                model.selection = None;
                drop(model);
                self.refresh();
            }
        }
        self.press.set(None);
        self.update_hover();
    }

    fn paint_stroke(&self, from: Point, to: Point) {
        let mut model = self.model.borrow_mut();
        let dab = match model.tool {
            Tool::Brush => Dab::Brush {
                color: hud::PALETTE[model.color],
            },
            Tool::Blur => Dab::Blur,
            Tool::Eraser => Dab::Erase,
            Tool::Select => return,
        };
        if matches!(dab, Dab::Blur) && model.blurred.is_none() {
            let radius = (model.shot.width.min(model.shot.height) / 80).clamp(8, 48);
            model.blurred = Some(model.shot.blurred(radius));
        }
        let radius = hud::RADII[model.radius];
        let model = &mut *model;
        model
            .annotations
            .stroke(from, to, radius, dab, model.blurred.as_ref());
        model.invalidate_annotations();
        self.refresh();
    }

    fn activate(&self, id: HudId) {
        match id {
            HudId::Tool(tool) => self.model.borrow_mut().tool = tool,
            HudId::Color(index) => self.model.borrow_mut().color = index,
            HudId::Radius(index) => self.model.borrow_mut().radius = index,
            HudId::Copy => self.copy_and_close(),
            HudId::Save => self.save_and_close(),
            HudId::SaveAs => self.save_as_and_close(),
            HudId::Close => self.close(),
        }
        self.refresh();
    }

    /// The region the next copy/save operates on: the selection, or the whole
    /// capture when nothing is selected.
    fn target(&self) -> Rect {
        let model = self.model.borrow();
        model.selection.unwrap_or(Rect {
            x: 0.0,
            y: 0.0,
            width: model.shot.width as f32,
            height: model.shot.height as f32,
        })
    }

    fn render(&self) -> Result<Shot, String> {
        let model = self.model.borrow();
        let target = self.target();
        let overlay = (!model.annotations.is_empty()).then_some(&model.annotations);
        model
            .shot
            .flattened(target, overlay)
            .ok_or_else(|| "the selected area is empty".to_owned())
    }

    fn copy_and_close(&self) {
        match self.render() {
            Ok(shot) => match arboard::Clipboard::new().and_then(|mut clipboard| {
                clipboard.set_image(arboard::ImageData {
                    width: shot.width as usize,
                    height: shot.height as usize,
                    bytes: shot.unpremultiplied().into(),
                })
            }) {
                Ok(()) => self.close(),
                Err(error) => self.fail(format!("could not copy to the clipboard: {error}")),
            },
            Err(error) => self.fail(error),
        }
    }

    fn save_and_close(&self) {
        match self.render() {
            Ok(shot) => {
                let path = paths::timestamped_target(&paths::screenshots_dir());
                match shot.write_png(&path) {
                    Ok(()) => self.close(),
                    Err(error) => self.fail(error),
                }
            }
            Err(error) => self.fail(error),
        }
    }

    fn save_as_and_close(&self) {
        let Some(app) = &self.app else {
            self.fail("file dialogs need a running application".into());
            return;
        };
        if self.finishing.replace(true) {
            return;
        }
        let state = self.clone();
        app.spawn_background(
            || {
                rfd::FileDialog::new()
                    .set_title("Save screenshot")
                    .set_directory(paths::screenshots_dir())
                    .set_file_name(
                        paths::timestamped_target(std::path::Path::new(""))
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .into_owned(),
                    )
                    .save_file()
            },
            move |path| {
                state.finishing.set(false);
                let Some(path) = path else { return };
                match state.render().and_then(|shot| {
                    let path = if path.extension().is_none() {
                        path.with_extension("png")
                    } else {
                        path
                    };
                    shot.write_png(&path)
                }) {
                    Ok(()) => state.close(),
                    Err(error) => state.fail(error),
                }
            },
        );
    }

    fn fail(&self, error: String) {
        eprintln!("coconut-screenshots: {error}");
        self.model.borrow_mut().status = Some(error);
        self.refresh();
    }

    fn close(&self) {
        if let Some(window) = self.window.borrow().as_ref() {
            window.close();
        } else if let Some(app) = &self.app {
            app.exit();
        }
    }

    fn select_all(&self) {
        let mut model = self.model.borrow_mut();
        model.selection = Some(Rect {
            x: 0.0,
            y: 0.0,
            width: model.shot.width as f32,
            height: model.shot.height as f32,
        });
        drop(model);
        self.refresh();
    }

    pub fn key(&self, event: KeyEvent, modifiers: Modifiers) -> bool {
        if !event.pressed {
            return false;
        }
        let ctrl = modifiers.ctrl || modifiers.logo;
        match &event.key {
            Key::Escape => {
                self.close();
                true
            }
            Key::Character(text) if ctrl => match text.to_ascii_lowercase().as_str() {
                "c" => {
                    self.copy_and_close();
                    true
                }
                "s" if modifiers.shift => {
                    self.save_as_and_close();
                    true
                }
                "s" => {
                    self.save_and_close();
                    true
                }
                "a" => {
                    self.select_all();
                    true
                }
                _ => false,
            },
            _ => false,
        }
    }

    pub fn build(&self, _: Size) -> creamui_core::BoxedWidget {
        self.revision.get();
        Box::new(canvas::Overlay {
            state: self.clone(),
        })
    }
}

/// `fit` scale and origin for `bounds` inside `viewport`.
fn transform(viewport: Rect, bounds: Size) -> (f32, Point) {
    if bounds.width <= 0.0 || bounds.height <= 0.0 {
        return (1.0, Point::default());
    }
    let scale = (viewport.width / bounds.width)
        .min(viewport.height / bounds.height)
        .max(f32::EPSILON);
    (
        scale,
        Point {
            x: viewport.x + (viewport.width - bounds.width * scale) / 2.0,
            y: viewport.y + (viewport.height - bounds.height * scale) / 2.0,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resize_handles_pull_their_edge() {
        let start = Rect {
            x: 10.0,
            y: 10.0,
            width: 40.0,
            height: 40.0,
        };
        let bounds = Size {
            width: 100.0,
            height: 100.0,
        };
        let resized = ResizeHandle::Se.apply(start, Point { x: 80.0, y: 60.0 }, bounds);
        assert_eq!(resized.width, 70.0);
        assert_eq!(resized.height, 50.0);
        let moved = ResizeHandle::Nw.apply(start, Point { x: 20.0, y: 5.0 }, bounds);
        assert_eq!(moved.x, 20.0);
        assert_eq!(moved.y, 5.0);
        assert_eq!(moved.width, 30.0);
        assert_eq!(moved.height, 45.0);
    }

    #[test]
    fn transform_centers_and_clamps_scale() {
        let viewport = Rect {
            x: 0.0,
            y: 0.0,
            width: 200.0,
            height: 100.0,
        };
        let (scale, origin) = transform(
            viewport,
            Size {
                width: 400.0,
                height: 100.0,
            },
        );
        assert_eq!(scale, 0.5);
        assert_eq!(origin.x, 0.0);
        assert_eq!(origin.y, 25.0);
    }
}
