use coconut_api::notifications::{
    Notification, NotificationEvent, NotificationListener, NotificationsIntegration,
};
use coconut_core::{PopupAlign, PopupConfig, PopupEdge, ShellConfig};
use coconut_plugin_kit::{
    chrome::{shell_border, shell_card},
    pixel_icon, IconRequest, IconResolver,
};
use creamui_core::{
    layout::{Dimension, LengthPercentageAuto, Position, Style},
    BoxedWidget, Rect, Size, Styled,
};
use creamui_image::ImageData;
use creamui_reactive::{create_effect, Effect, Signal};
use creamui_render::{
    platform::WindowRole, AppHandle, BlurRegion, PopupOptions, WindowHandle, WindowOptions,
};
use creamui_theme::{use_theme, Color};
use creamui_widgets::{
    layout::{Align, Flex, Justify},
    Button, ButtonSize, ButtonState, ButtonVariant, Icon, IconImage, IconSource, ProgressBar,
    RawButton, RawView, Symbol, Text, TextSize,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::{Duration, Instant},
};

#[derive(Clone, PartialEq)]
struct StatusUpdate {
    icon: &'static str,
    label: &'static str,
    level: f32,
    changed_at: Instant,
}

pub(crate) struct ShellNotification {
    pub icon: &'static str,
    pub summary: String,
    pub body: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NotificationId {
    Application(u32),
    Shell(u64),
}

impl NotificationId {
    fn close(self, backend: &dyn NotificationsIntegration, reason: u32) {
        if let Self::Application(id) = self {
            backend.close(id, reason);
        }
    }
}

#[derive(Clone)]
enum NotificationIcon {
    Image(IconSource),
    Bundled(&'static str),
}

#[derive(Clone)]
struct ActiveNotification {
    id: NotificationId,
    notification: Rc<Notification>,
    icon: Option<NotificationIcon>,
    changed_at: Instant,
}

pub(crate) struct PopupHost {
    app: AppHandle,
    config: Signal<ShellConfig>,
    backend: Rc<dyn NotificationsIntegration>,
    status: Signal<Option<StatusUpdate>>,
    notifications: Signal<Vec<ActiveNotification>>,
    window: RefCell<Option<WindowHandle>>,
    pending_window: RefCell<bool>,
    effects: RefCell<Vec<Effect>>,
    viewport: Cell<Option<Size>>,
    notification_window: RefCell<Option<WindowHandle>>,
    notification_geometry: Cell<Option<Rect>>,
    pending_notification: Cell<bool>,
    next_shell_id: Cell<u64>,
    #[cfg(test)]
    notification_window_count: Cell<usize>,
}

impl PopupHost {
    pub(crate) fn start(
        app: AppHandle,
        config: Signal<ShellConfig>,
        volume_level: Signal<f32>,
        brightness_level: Signal<f32>,
        volume: Rc<dyn coconut_api::volume::VolumeIntegration>,
        backend: Rc<dyn NotificationsIntegration>,
    ) -> Rc<Self> {
        let host = Rc::new(Self {
            app,
            config,
            backend,
            status: Signal::new(None),
            notifications: Signal::new(Vec::new()),
            window: RefCell::new(None),
            pending_window: RefCell::new(false),
            effects: RefCell::new(Vec::new()),
            viewport: Cell::new(None),
            notification_window: RefCell::new(None),
            notification_geometry: Cell::new(None),
            pending_notification: Cell::new(false),
            next_shell_id: Cell::new(0),
            #[cfg(test)]
            notification_window_count: Cell::new(0),
        });
        let weak = Rc::downgrade(&host);
        let mut previous_volume = (volume_level.peek(), volume.muted());
        let mut previous_brightness = brightness_level.peek();
        host.effects.borrow_mut().push(create_effect(move || {
            let next_volume = (volume_level.get(), volume.muted());
            let next_brightness = brightness_level.get();
            if let Some(host) = weak.upgrade() {
                if previous_volume != next_volume {
                    host.show_status(
                        if next_volume.1 || next_volume.0 <= 0.01 {
                            "volume-mute"
                        } else if next_volume.0 < 0.34 {
                            "volume-low"
                        } else if next_volume.0 < 0.67 {
                            "volume-mid"
                        } else {
                            "volume-high"
                        },
                        if next_volume.1 { "Muted" } else { "Volume" },
                        next_volume.0,
                    );
                }
                if previous_brightness != next_brightness {
                    host.show_status("brightness", "Brightness", next_brightness);
                }
            }
            previous_volume = next_volume;
            previous_brightness = next_brightness;
        }));
        let weak = Rc::downgrade(&host);
        host.effects.borrow_mut().push(create_effect(move || {
            if let Some(host) = weak.upgrade() {
                let config = host.config.get();
                let status = host.status.get();
                let notifications = host.notifications.get();
                if !config.status_updates.enabled && status.is_some() {
                    host.status.set(None);
                }
                if !config.notifications.enabled && !notifications.is_empty() {
                    for entry in notifications {
                        entry.id.close(host.backend.as_ref(), 2);
                    }
                    host.notifications.set(Vec::new());
                }
                host.update_window();
            }
        }));
        if let Some(listener) = host.backend.start() {
            wait_for_notification(host.clone(), listener);
        }
        schedule_expiration(host.clone());
        host
    }

    fn show_status(&self, icon: &'static str, label: &'static str, level: f32) {
        if self.config.peek().status_updates.enabled {
            self.status.set(Some(StatusUpdate {
                icon,
                label,
                level,
                changed_at: Instant::now(),
            }));
        }
    }

    pub(crate) fn watch_connectivity(
        self: &Rc<Self>,
        network: Rc<dyn coconut_api::network::NetworkIntegration>,
        bluetooth: Rc<dyn coconut_api::bluetooth::BluetoothIntegration>,
        network_revision: Signal<()>,
        bluetooth_revision: Signal<()>,
    ) {
        self.effects.borrow_mut().extend(crate::connectivity::watch(
            self,
            network,
            bluetooth,
            network_revision,
            bluetooth_revision,
        ));
    }

    pub(crate) fn watch_battery(
        self: &Rc<Self>,
        battery: Rc<dyn coconut_api::battery::BatteryIntegration>,
        battery_revision: Signal<()>,
    ) {
        self.effects
            .borrow_mut()
            .push(crate::battery::watch(self, battery, battery_revision));
    }

    pub(crate) fn notify(&self, notification: ShellNotification) {
        let id = self.next_shell_id.get().wrapping_add(1);
        self.next_shell_id.set(id);
        self.show_notification(ActiveNotification {
            id: NotificationId::Shell(id),
            notification: Rc::new(Notification {
                id: 0,
                app_name: "Coconut".into(),
                icon: String::new(),
                image: None,
                summary: notification.summary,
                body: notification.body,
                urgency: 1,
                actions: Vec::new(),
                resident: false,
                expire_timeout: -1,
            }),
            icon: Some(NotificationIcon::Bundled(notification.icon)),
            changed_at: Instant::now(),
        });
    }

    pub(crate) fn battery_alerts(&self) -> coconut_core::BatteryAlertsConfig {
        self.config.get().battery_alerts
    }

    fn show_notification(&self, entry: ActiveNotification) {
        if !self.config.peek().notifications.enabled {
            entry.id.close(self.backend.as_ref(), 2);
            return;
        }
        self.notifications.update(|entries| {
            if let Some(existing) = entries.iter_mut().find(|old| old.id == entry.id) {
                *existing = entry;
            } else {
                if entries.len() >= 5 {
                    entries.remove(0).id.close(self.backend.as_ref(), 2);
                }
                entries.push(entry);
            }
        });
    }

    fn receive(&self, event: NotificationEvent) {
        match event {
            NotificationEvent::Show(mut notification) => {
                if !self.config.peek().notifications.enabled {
                    self.backend.close(notification.id, 2);
                    return;
                }
                let resolver = IconResolver::new(self.config.peek().appearance.icon_theme.clone());
                let icon = notification
                    .image
                    .take()
                    .and_then(|image| {
                        ImageData::from_rgba(image.width, image.height, image.pixels).ok()
                    })
                    .or_else(|| {
                        let name = notification
                            .icon
                            .strip_prefix("file://")
                            .unwrap_or(&notification.icon);
                        resolver.load(IconRequest::named(name, 40))
                    })
                    .map(|image| {
                        NotificationIcon::Image(IconSource::Image(IconImage {
                            image: image.image().clone(),
                            monochrome: false,
                        }))
                    });
                self.show_notification(ActiveNotification {
                    id: NotificationId::Application(notification.id),
                    notification: Rc::new(notification),
                    icon,
                    changed_at: Instant::now(),
                });
            }
            NotificationEvent::Close(id) => self.notifications.update(|entries| {
                entries.retain(|entry| entry.id != NotificationId::Application(id))
            }),
        }
    }

    fn expire(&self) {
        let config = self.config.peek();
        if self.status.peek().is_some_and(|status| {
            status.changed_at.elapsed()
                >= Duration::from_millis(config.status_updates.duration_ms.max(100) as u64)
        }) {
            self.status.set(None);
        }
        let entries = self.notifications.peek();
        let expired: Vec<_> = entries
            .iter()
            .filter(|entry| {
                notification_duration(&entry.notification, config.notifications.duration_ms)
                    .is_some_and(|duration| entry.changed_at.elapsed() >= duration)
            })
            .map(|entry| entry.id)
            .collect();
        if !expired.is_empty() {
            for id in &expired {
                id.close(self.backend.as_ref(), 1);
            }
            self.notifications
                .update(|entries| entries.retain(|entry| !expired.contains(&entry.id)));
        }
    }

    fn update_window(self: &Rc<Self>) {
        let visible = self.status.peek().is_some() || !self.notifications.peek().is_empty();
        if !visible {
            self.close_notification_window();
            if self.pending_notification.get() {
                return;
            }
            if let Some(window) = self.window.borrow_mut().take() {
                window.close();
            }
            return;
        }
        if self
            .window
            .borrow()
            .as_ref()
            .is_some_and(WindowHandle::is_open)
        {
            self.update_notifications();
            return;
        }
        if *self.pending_window.borrow() {
            return;
        }
        *self.pending_window.borrow_mut() = true;
        self.viewport.set(None);
        let ready = self.clone();
        let build = self.clone();
        self.app.append_window(
            WindowOptions {
                title: "Coconut Status".into(),
                width: if cfg!(target_os = "linux") { 1 } else { 1280 },
                height: if cfg!(target_os = "linux") { 1 } else { 720 },
                decorations: false,
                resizable: false,
                transparent: true,
                blur: Some(BlurRegion::Content),
                role: WindowRole::Overlay,
                theme: super::system_theme(),
                ..Default::default()
            },
            Color::rgba(0, 0, 0, 0),
            move |window| {
                window.set_always_on_top(true);
                *ready.pending_window.borrow_mut() = false;
                *ready.window.borrow_mut() = Some(window);
                ready.update_window();
            },
            move |size| build.build(size),
        );
    }

    pub(crate) fn set_theme(&self, theme: creamui_theme::Theme) {
        if let Some(window) = self.window.borrow().as_ref() {
            window.set_theme(theme);
        }
        if let Some(window) = self.notification_window.borrow().as_ref() {
            window.set_theme(theme);
        }
    }

    fn build(self: &Rc<Self>, size: Size) -> BoxedWidget {
        if size.width <= 1.0 || size.height <= 1.0 {
            return Box::new(RawView::new(Style::default()));
        }
        self.viewport.set(Some(size));
        self.update_notifications();
        build_overlay(size, &self.config.get(), self.status.get().as_ref())
    }

    fn close_notification_window(&self) {
        if let Some(window) = self.notification_window.borrow_mut().take() {
            window.close();
        }
        self.notification_geometry.set(None);
    }

    fn update_notifications(self: &Rc<Self>) {
        let entries = self.notifications.peek();
        if entries.is_empty() {
            self.close_notification_window();
            return;
        }
        if self.pending_notification.get() {
            return;
        }
        let Some(parent) = self.window.borrow().clone() else {
            return;
        };
        let Some(size) = self.viewport.get() else {
            return;
        };
        let count = visible_count(size, &entries);
        let width = 380.0f32.min(size.width);
        let height = notification_stack_height(entries.iter().rev().take(count));
        let (x, y) = placement(size, &self.config.peek().notifications, width, height);
        let geometry = Rect {
            x,
            y,
            width,
            height,
        };
        if self.notification_geometry.get() == Some(geometry)
            && self
                .notification_window
                .borrow()
                .as_ref()
                .is_some_and(WindowHandle::is_open)
        {
            return;
        }
        self.close_notification_window();
        self.notification_geometry.set(Some(geometry));
        self.pending_notification.set(true);
        #[cfg(test)]
        self.notification_window_count
            .set(self.notification_window_count.get() + 1);
        let ready = self.clone();
        let build = self.clone();
        self.app.append_popup(
            WindowOptions {
                title: "Coconut Notifications".into(),
                width: width.max(1.0) as u32,
                height: height.max(1.0) as u32,
                transparent: true,
                blur: Some(BlurRegion::Content),
                decorations: false,
                resizable: false,
                theme: super::system_theme(),
                ..Default::default()
            },
            PopupOptions::new(
                parent,
                Rect {
                    x,
                    y: y - 1.0,
                    width,
                    height: 1.0,
                },
            )
            .below(),
            Color::rgba(0, 0, 0, 0),
            move |window| {
                ready.pending_notification.set(false);
                *ready.notification_window.borrow_mut() = Some(window);
                ready.update_window();
            },
            move |_| build.notification_stack(count, width),
        );
    }

    fn dismiss(&self, id: NotificationId) {
        id.close(self.backend.as_ref(), 2);
        self.notifications
            .update(|entries| entries.retain(|entry| entry.id != id));
    }

    fn invoke(&self, id: NotificationId, action: &str) {
        if let Some(entry) = self
            .notifications
            .peek()
            .iter()
            .find(|entry| entry.id == id)
        {
            if let NotificationId::Application(id) = id {
                self.backend.invoke(id, action);
            }
            if !entry.notification.resident {
                self.dismiss(id);
            }
        }
    }

    fn notification_stack(self: &Rc<Self>, count: usize, width: f32) -> BoxedWidget {
        let mut stack = Flex::column().gap(12.0);
        for entry in self.notifications.get().iter().rev().take(count) {
            let dismiss = self.clone();
            let invoke = self.clone();
            let id = entry.id;
            stack = stack.child(notification_card(
                entry,
                width,
                Rc::new(move || dismiss.dismiss(id)),
                Rc::new(move |key| invoke.invoke(id, &key)),
            ));
        }
        Box::new(stack)
    }
}

fn notification_duration(notification: &Notification, default_ms: u32) -> Option<Duration> {
    match notification.expire_timeout {
        0 => None,
        n if n > 0 => Some(Duration::from_millis(n as u64)),
        _ if notification.urgency >= 2 => None,
        _ => Some(Duration::from_millis(default_ms.max(100) as u64)),
    }
}

fn wait_for_notification(host: Rc<PopupHost>, listener: NotificationListener) {
    let next_listener = listener.clone();
    host.app.clone().spawn_background(
        move || listener.wait(),
        move |event| {
            if let Some(event) = event {
                host.receive(event);
                wait_for_notification(host, next_listener);
            }
        },
    );
}

fn schedule_expiration(host: Rc<PopupHost>) {
    host.app.clone().spawn_background(
        || std::thread::sleep(Duration::from_millis(100)),
        move |_| {
            host.expire();
            host.update_window();
            schedule_expiration(host);
        },
    );
}

fn placement(size: Size, config: &PopupConfig, width: f32, height: f32) -> (f32, f32) {
    let offset = if config.offset.is_finite() {
        config.offset.max(0.0)
    } else {
        0.0
    };
    let max_x = (size.width - width).max(0.0);
    let max_y = (size.height - height).max(0.0);
    let x = match config.align {
        PopupAlign::Left => offset.min(max_x),
        PopupAlign::Center => max_x / 2.0,
        PopupAlign::Right => (max_x - offset).max(0.0),
    };
    let y = match config.edge {
        PopupEdge::Top => offset.min(max_y),
        PopupEdge::Bottom => (max_y - offset).max(0.0),
    };
    (x, y)
}

fn positioned(
    child: BoxedWidget,
    size: Size,
    config: &PopupConfig,
    width: f32,
    height: f32,
) -> BoxedWidget {
    let (x, y) = placement(size, config, width, height);
    Box::new(
        RawView::new(Style {
            position: Position::Absolute,
            inset: creamui_core::layout::Rect {
                left: LengthPercentageAuto::Length(x),
                top: LengthPercentageAuto::Length(y),
                right: LengthPercentageAuto::Auto,
                bottom: LengthPercentageAuto::Auto,
            },
            size: creamui_core::layout::Size {
                width: Dimension::Length(width),
                height: Dimension::Length(height),
            },
            ..Default::default()
        })
        .child(child),
    )
}

fn status_card(status: &StatusUpdate, width: f32) -> BoxedWidget {
    Box::new(
        Flex::row()
            .align(Align::Center)
            .width(width)
            .height(64.0)
            .padding(14.0)
            .gap(12.0)
            .background(shell_card())
            .border(shell_border(), 1.0)
            .corner_radius(18.0)
            .child(pixel_icon(status.icon, 24.0, use_theme().colors.text_primary))
            .child(Box::new(
                ProgressBar::new(if status.label == "Muted" {
                    0.0
                } else {
                    status.level.clamp(0.0, 1.0)
                })
                .width((width - 98.0).max(0.0))
                .height(6.0),
            ))
            .child(Box::new(Text::secondary(format!("{:.0}%", status.level * 100.0)))),
    )
}

fn notification_card_height(entry: &ActiveNotification) -> f32 {
    if matches!(entry.id, NotificationId::Shell(_)) {
        96.0
    } else {
        196.0
    }
}

fn notification_card(
    entry: &ActiveNotification,
    width: f32,
    dismiss: Rc<dyn Fn()>,
    invoke: Rc<dyn Fn(String)>,
) -> BoxedWidget {
    if matches!(entry.id, NotificationId::Shell(_)) {
        return compact_notification_card(entry, width, dismiss);
    }
    let text_width = (width - 88.0).max(1.0);
    let clamp = |text: &str, size, lines| {
        creamui_widgets::clamp_to_lines(text, size, text_width, None, lines)
    };
    let mut text = Flex::column().grow(1.0).gap(6.0).child(Box::new(
        Text::new(clamp(&entry.notification.summary, 14.0, 2)).bold(true),
    ));
    if !entry.notification.body.is_empty() {
        text = text.child(Box::new(
            Text::secondary(clamp(&entry.notification.body, 12.0, 3)).size(TextSize::Sm),
        ));
    }
    let mut actions = Flex::row().gap(8.0);
    for (key, label) in entry.notification.actions.iter().take(3) {
        let invoke = invoke.clone();
        let key = key.clone();
        let label = if label.is_empty() {
            "Open".to_owned()
        } else {
            clamp(label, 11.0, 1)
        };
        actions = actions.child(Box::new(Button::styled(
            ButtonVariant::Secondary,
            ButtonSize::Xs,
            label,
            ButtonState::Normal,
            move || invoke(key.clone()),
        )));
    }
    Box::new(
        Flex::column()
            .width(width)
            .height(196.0)
            .padding(16.0)
            .gap(10.0)
            .background(shell_card())
            .border(shell_border(), 1.0)
            .corner_radius(18.0)
            .child(Box::new(
                Flex::row()
                    .justify(Justify::Between)
                    .align(Align::Center)
                    .child(Box::new(
                        Text::secondary(clamp(&entry.notification.app_name, 11.0, 1))
                            .size(TextSize::Xs),
                    ))
                    .child(Box::new(
                        RawButton::new(
                            Style {
                                size: creamui_widgets::layout::fixed(20.0, 20.0),
                                ..Default::default()
                            },
                            move || {
                                dismiss();
                            },
                        )
                        .child(Box::new(
                            Icon::new(Symbol::Close, use_theme().colors.text_secondary).size(16.0),
                        )),
                    )),
            ))
            .child(Box::new(
                Flex::row()
                    .gap(12.0)
                    .grow(1.0)
                    .child(match &entry.icon {
                        Some(NotificationIcon::Bundled(name)) => {
                            pixel_icon(name, 40.0, use_theme().colors.text_primary)
                        }
                        _ => Box::new(
                            Icon::new(
                                match &entry.icon {
                                    Some(NotificationIcon::Image(source)) => source.clone(),
                                    _ => IconSource::Initial {
                                        letter: entry
                                            .notification
                                            .app_name
                                            .chars()
                                            .next()
                                            .unwrap_or('N')
                                            .to_ascii_uppercase(),
                                        background: use_theme().colors.selection_background,
                                        text_color: use_theme().colors.text_primary,
                                    },
                                },
                                use_theme().colors.text_primary,
                            )
                            .size(40.0),
                        ),
                    })
                    .child(Box::new(text)),
            ))
            .child(Box::new(actions)),
    )
}

fn compact_notification_card(
    entry: &ActiveNotification,
    width: f32,
    dismiss: Rc<dyn Fn()>,
) -> BoxedWidget {
    let text_width = (width - 92.0).max(1.0);
    let icon = match &entry.icon {
        Some(NotificationIcon::Bundled(name)) => {
            pixel_icon(name, 32.0, use_theme().colors.text_primary)
        }
        Some(NotificationIcon::Image(source)) => Box::new(
            Icon::new(source.clone(), use_theme().colors.text_primary).size(32.0),
        ),
        None => Box::new(
            Icon::new(
                IconSource::Initial {
                    letter: entry
                        .notification
                        .app_name
                        .chars()
                        .next()
                        .unwrap_or('N')
                        .to_ascii_uppercase(),
                    background: use_theme().colors.selection_background,
                    text_color: use_theme().colors.text_primary,
                },
                use_theme().colors.text_primary,
            )
            .size(32.0),
        ),
    };
    let mut text = Flex::column().grow(1.0).gap(4.0).child(Box::new(
        Text::new(creamui_widgets::clamp_to_lines(
            &entry.notification.summary,
            14.0,
            text_width,
            None,
            1,
        ))
        .bold(true),
    ));
    if !entry.notification.body.is_empty() {
        text = text.child(Box::new(
            Text::secondary(creamui_widgets::clamp_to_lines(
                &entry.notification.body,
                12.0,
                text_width,
                None,
                1,
            ))
            .size(TextSize::Sm),
        ));
    }
    Box::new(
        Flex::row()
            .align(Align::Center)
            .width(width)
            .height(notification_card_height(entry))
            .padding(16.0)
            .gap(12.0)
            .background(shell_card())
            .border(shell_border(), 1.0)
            .corner_radius(18.0)
            .child(icon)
            .child(Box::new(text))
            .child(Box::new(
                RawButton::new(
                    Style {
                        size: creamui_widgets::layout::fixed(20.0, 20.0),
                        ..Default::default()
                    },
                    move || dismiss(),
                )
                .child(Box::new(
                    Icon::new(Symbol::Close, use_theme().colors.text_secondary).size(16.0),
                )),
            )),
    )
}

fn visible_count(size: Size, entries: &[ActiveNotification]) -> usize {
    let mut height = 0.0;
    let mut count = 0;
    for entry in entries.iter().rev() {
        let next_height = notification_card_height(entry) + if count == 0 { 0.0 } else { 12.0 };
        if count > 0 && height + next_height > size.height {
            break;
        }
        height += next_height;
        count += 1;
    }
    count.max(1)
}

fn notification_stack_height<'a>(entries: impl Iterator<Item = &'a ActiveNotification>) -> f32 {
    entries
        .enumerate()
        .map(|(index, entry)| notification_card_height(entry) + if index == 0 { 0.0 } else { 12.0 })
        .sum()
}

fn build_overlay(size: Size, config: &ShellConfig, status: Option<&StatusUpdate>) -> BoxedWidget {
    let mut root = RawView::new(Style {
        size: creamui_core::layout::Size {
            width: Dimension::Length(size.width),
            height: Dimension::Length(size.height),
        },
        ..Default::default()
    });
    if let Some(status) = status {
        let width = 240.0f32.min(size.width);
        root = root.child(positioned(
            status_card(status, width),
            size,
            &config.status_updates,
            width,
            64.0,
        ));
    }
    Box::new(root)
}

#[cfg(test)]
mod tests {
    use super::*;
    use creamui_core::{Point, Renderer};
    use creamui_render::{Damage, Rasterizer, SceneRecorder};
    use creamui_theme::{Theme, ThemeProvider};

    fn notification() -> Notification {
        Notification {
            id: 1,
            app_name: "Files".into(),
            icon: String::new(),
            image: None,
            summary: "Transfer complete".into(),
            body: "All your files have been copied to the external drive.".into(),
            urgency: 1,
            actions: vec![("open".into(), "Open folder".into())],
            resident: false,
            expire_timeout: -1,
        }
    }

    #[test]
    fn respects_client_timeouts_and_critical_notifications() {
        let mut n = notification();
        assert_eq!(
            notification_duration(&n, 5000),
            Some(Duration::from_secs(5))
        );
        n.urgency = 2;
        assert_eq!(notification_duration(&n, 5000), None);
        n.expire_timeout = 100;
        assert_eq!(
            notification_duration(&n, 5000),
            Some(Duration::from_millis(100))
        );
        n.expire_timeout = 0;
        assert_eq!(notification_duration(&n, 5000), None);
    }

    #[test]
    fn all_positions_stay_on_screen_with_large_offsets() {
        for size in [
            Size {
                width: 1920.0,
                height: 1080.0,
            },
            Size {
                width: 320.0,
                height: 240.0,
            },
        ] {
            for edge in [PopupEdge::Top, PopupEdge::Bottom] {
                for align in [PopupAlign::Left, PopupAlign::Center, PopupAlign::Right] {
                    for offset in [64.0, 120.0, 9000.0, -10.0, f32::NAN] {
                        let config = PopupConfig {
                            edge,
                            align,
                            offset,
                            ..Default::default()
                        };
                        let (x, y) = placement(size, &config, 280.0, 88.0);
                        assert!(
                            x >= 0.0
                                && y >= 0.0
                                && x + 280.0 <= size.width
                                && y + 88.0 <= size.height
                        );
                    }
                }
            }
        }
        assert_eq!(
            placement(
                Size {
                    width: 1920.0,
                    height: 1080.0
                },
                &PopupConfig::default(),
                280.0,
                88.0
            ),
            (820.0, 872.0)
        );
    }

    #[test]
    fn floating_cards_render_and_notification_controls_are_clickable() {
        for base in [Theme::light(), Theme::dark()] {
            creamui_reactive::with_context_scope(|| {
                coconut_plugin_kit::design::load_fonts();
                let theme = coconut_plugin_kit::design::coconut_theme(base);
                creamui_reactive::provide_context(ThemeProvider::new(theme));
                let dismissed = Rc::new(Cell::new(false));
                let invoked = Rc::new(Cell::new(false));
                let entry = ActiveNotification {
                    id: NotificationId::Application(1),
                    notification: Rc::new(notification()),
                    icon: None,
                    changed_at: Instant::now(),
                };
                let dismiss = dismissed.clone();
                let invoke = invoked.clone();
                let card = notification_card(
                    &entry,
                    380.0,
                    Rc::new(move || dismiss.set(true)),
                    Rc::new(move |key| {
                        assert_eq!(key, "open");
                        invoke.set(true);
                    }),
                );
                let mut recorder = SceneRecorder::new();
                recorder.begin(380, 196, 1.0, Color::rgba(0, 0, 0, 0), theme.colors);
                let scene = Renderer::new().render(
                    card,
                    Size {
                        width: 380.0,
                        height: 196.0,
                    },
                    &mut recorder,
                );
                let mut raster = Rasterizer::new(380, 196);
                raster.render(&recorder.finish(), &Damage::Full);
                assert!(raster.pixmap().pixel(180, 90).unwrap().alpha() > 0);
                scene
                    .hit_test(Point { x: 353.0, y: 26.0 })
                    .expect("dismiss button")();
                assert!(dismissed.get());
                scene
                    .hit_test(Point { x: 58.0, y: 166.0 })
                    .expect("notification action")();
                assert!(invoked.get());
                let path = std::env::temp_dir().join(format!(
                    "coconut-notification-{}.png",
                    if base == Theme::light() {
                        "light"
                    } else {
                        "dark"
                    }
                ));
                raster.pixmap().save_png(path).unwrap();
                let status = StatusUpdate {
                    icon: "volume-high",
                    label: "Volume",
                    level: 0.42,
                    changed_at: Instant::now(),
                };
                let config = ShellConfig::default();
                recorder.begin(1280, 720, 1.0, Color::rgba(0, 0, 0, 0), theme.colors);
                let scene = Renderer::new().render(
                    build_overlay(
                        Size {
                            width: 1280.0,
                            height: 720.0,
                        },
                        &config,
                        Some(&status),
                    ),
                    Size {
                        width: 1280.0,
                        height: 720.0,
                    },
                    &mut recorder,
                );
                let mut raster = Rasterizer::new(1280, 720);
                raster.render(&recorder.finish(), &Damage::Full);
                assert_eq!(raster.pixmap().pixel(0, 0).unwrap().alpha(), 0);
                assert!(raster.pixmap().pixel(640, 540).unwrap().alpha() > 0);
                assert!(scene.hit_test(Point { x: 640.0, y: 540.0 }).is_none());
                raster
                    .pixmap()
                    .save_png(std::env::temp_dir().join("coconut-status-overlay.png"))
                    .unwrap();
            });
        }
    }

    #[derive(Default)]
    struct RecordingNotifications {
        closed: RefCell<Vec<(u32, u32)>>,
    }

    impl NotificationsIntegration for RecordingNotifications {
        fn start(&self) -> Option<NotificationListener> {
            None
        }
        fn close(&self, id: u32, reason: u32) {
            self.closed.borrow_mut().push((id, reason));
        }
        fn invoke(&self, _: u32, _: &str) {
            panic!("shell notifications do not invoke bus actions");
        }
    }

    struct Network(Cell<bool>);

    impl coconut_api::network::NetworkIntegration for Network {
        fn enabled(&self) -> bool {
            true
        }
        fn connected(&self) -> bool {
            self.0.get()
        }
        fn network_name(&self) -> Option<String> {
            self.connected().then(|| "Home".into())
        }
        fn strength(&self) -> Option<u8> {
            Some(90)
        }
        fn set_enabled(&self, _: bool) {}
    }

    #[test]
    #[ignore = "requires an isolated Wayland compositor"]
    fn native_popups_reconfigure_and_expire() {
        creamui_render::AppBuilder::new()
            .keep_running()
            .on_started(|app| {
                let config = Signal::new(ShellConfig::default());
                let level = Signal::new(0.4);
                let backend = Rc::new(RecordingNotifications::default());
                let host = PopupHost::start(
                    app.clone(),
                    config,
                    level.clone(),
                    Signal::new(0.7),
                    Rc::new(coconut_api::volume::Fallback),
                    backend.clone(),
                );
                assert!(host.status.peek().is_none());
                level.set(0.6);
                let mut notification = notification();
                notification.expire_timeout = 0;
                host.receive(NotificationEvent::Show(notification));
                native_stage(host, level, backend, 0);
            })
            .run();
    }

    fn native_stage(
        host: Rc<PopupHost>,
        level: Signal<f32>,
        backend: Rc<RecordingNotifications>,
        stage: usize,
    ) {
        host.app.clone().spawn_background(
            || std::thread::sleep(Duration::from_millis(300)),
            move |_| {
                match stage {
                    0 => {
                        assert_eq!(
                            host.notification_window_count.get(),
                            1,
                            "the first popup already has its final position"
                        );
                        assert!(host
                            .window
                            .borrow()
                            .as_ref()
                            .is_some_and(WindowHandle::is_open));
                        assert!(host
                            .notification_window
                            .borrow()
                            .as_ref()
                            .is_some_and(WindowHandle::is_open));
                        let geometry = host.notification_geometry.get();
                        let mut updated = notification();
                        updated.expire_timeout = 0;
                        updated.summary = "Updated without reopening".into();
                        host.receive(NotificationEvent::Show(updated));
                        assert_eq!(host.notification_geometry.get(), geometry);
                        host.config.update(|config| {
                            config.status_updates.duration_ms = 100;
                            config.notifications.edge = PopupEdge::Bottom;
                            config.notifications.align = PopupAlign::Left;
                        });
                    }
                    1 => {
                        assert!(host.status.peek().is_none());
                        assert_eq!(host.notifications.peek().len(), 1);
                        let network = Rc::new(Network(Cell::new(false)));
                        let revision = Signal::new(());
                        host.watch_connectivity(
                            network.clone(),
                            Rc::new(coconut_api::bluetooth::Fallback),
                            revision.clone(),
                            Signal::new(()),
                        );
                        revision.set(());
                        assert_eq!(
                            host.notifications.peek().len(),
                            1,
                            "initial state is silent"
                        );
                        network.0.set(true);
                        revision.set(());
                        revision.set(());
                        assert_eq!(
                            host.notifications.peek().len(),
                            2,
                            "one native connection notice"
                        );
                        let entries = host.notifications.peek();
                        let shell = entries
                            .iter()
                            .find(|entry| matches!(entry.id, NotificationId::Shell(_)))
                            .unwrap();
                        assert_eq!(shell.notification.body, "Connected to Home.");
                        assert_eq!(shell.id, NotificationId::Shell(1));
                        host.receive(NotificationEvent::Close(1));
                        assert_eq!(
                            host.notifications.peek().len(),
                            1,
                            "bus close does not touch a shell notice"
                        );
                        host.dismiss(shell.id);
                        assert!(host.notifications.peek().is_empty());
                        host.notify(ShellNotification {
                            icon: "bluetooth-connected",
                            summary: "Bluetooth connected".into(),
                            body: "Connected to Headphones.".into(),
                        });
                        host.notifications.update(|entries| {
                            entries[0].changed_at = Instant::now() - Duration::from_secs(10)
                        });
                        host.expire();
                        assert!(host.notifications.peek().is_empty());
                        assert!(
                            backend.closed.borrow().is_empty(),
                            "native dismissal and expiry stay off the bus"
                        );
                        let mut restored = notification();
                        restored.expire_timeout = 0;
                        host.receive(NotificationEvent::Show(restored));
                        for id in [2, 3] {
                            let mut n = notification();
                            n.id = id;
                            n.expire_timeout = 0;
                            host.receive(NotificationEvent::Show(n));
                        }
                        level.set(0.7);
                    }
                    2 => {
                        assert!(host
                            .notification_window
                            .borrow()
                            .as_ref()
                            .is_some_and(WindowHandle::is_open));
                        host.config.update(|config| {
                            config.status_updates.enabled = false;
                            config.notifications.enabled = false;
                        });
                        assert!(host.notifications.peek().is_empty());
                    }
                    3 => {
                        assert!(host.window.borrow().is_none());
                        assert!(host.notification_window.borrow().is_none());
                        host.config
                            .update(|config| config.notifications.enabled = true);
                        let mut n = notification();
                        n.expire_timeout = 100;
                        host.receive(NotificationEvent::Show(n));
                    }
                    4 => {
                        assert!(host.notifications.peek().is_empty());
                        assert!(host.window.borrow().is_none());
                        assert!(host.notification_window.borrow().is_none());
                        host.app.exit();
                        return;
                    }
                    _ => unreachable!(),
                }
                native_stage(host, level, backend, stage + 1);
            },
        );
    }
}
