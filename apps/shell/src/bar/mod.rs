use coconut_core::{
    ordered_islands, BackgroundSource, DesktopColor, DockAlign, DockConfig, DockPosition,
    IslandEntry, SectionConfig,
};
use coconut_plugin_kit::chrome::ISLAND_RADIUS;
use coconut_plugin_kit::{
    apply_opacity, parse_gap, with_island_chrome, ConfigValue, Gap, Island, IslandChrome,
    IslandConfig, IslandRenderContext, PluginRegistry, SharedState,
};
use creamui_core::{BoxedWidget, Point, Size, Styled};
use creamui_reactive::Signal;
use creamui_theme::{use_theme, Color};
use creamui_widgets::layout::{Align, Flex, Justify};
use std::collections::HashMap;
use std::rc::Rc;

/// Bumped whenever a `modules/<id>.toml` changes on disk.
#[derive(Clone)]
pub struct ModuleConfigRevision(pub Signal<u64>);

/// A dock's configured [`DockConfig::thickness`], floored so an accidental
/// zero/negative value never collapses the dock to nothing.
fn bar_thickness(dock: &DockConfig) -> f32 {
    dock.thickness.max(1.0)
}

/// How much bigger or smaller than the default thickness this dock is —
/// islands multiply their own fixed pixel sizes by this to fill it.
fn island_scale(dock: &DockConfig) -> f32 {
    bar_thickness(dock) / coconut_core::DEFAULT_THICKNESS
}

fn dock_background(dock: &DockConfig) -> Color {
    let base = resolve_background_color(
        dock.background_source,
        dock.background_color,
        use_theme().colors.surface,
    );
    apply_opacity(base, dock.background_opacity)
}

fn island_background_color(dock: &DockConfig) -> Option<Color> {
    Some(resolve_background_color(
        dock.island_background_source,
        dock.island_background_color,
        use_theme().colors.surface_elevated,
    ))
}

fn resolve_background_color(
    source: BackgroundSource,
    custom: DesktopColor,
    theme_default: Color,
) -> Color {
    match source {
        BackgroundSource::Theme => theme_default,
        BackgroundSource::Custom => Color::rgb(custom.r, custom.g, custom.b),
    }
}

fn dock_border() -> Color {
    use_theme().colors.border
}

/// Builds one dock's full widget tree: its sections, laid out along the
/// dock's position (row for top/bottom, column for left/right), each
/// containing its configured islands in `index` order.
pub fn build_dock(
    viewport: Size,
    dock: &DockConfig,
    registry: &PluginRegistry,
    shared: &SharedState,
    open_panel: &Rc<dyn Fn(&'static str, Point)>,
) -> BoxedWidget {
    if dock.position.is_vertical() {
        build_vertical_dock(viewport, dock, registry, shared, open_panel)
    } else {
        build_horizontal_dock(viewport, dock, registry, shared, open_panel)
    }
}

fn build_horizontal_dock(
    viewport: Size,
    dock: &DockConfig,
    registry: &PluginRegistry,
    shared: &SharedState,
    open_panel: &Rc<dyn Fn(&'static str, Point)>,
) -> BoxedWidget {
    let sections: Vec<&SectionConfig> = dock.sections.iter().filter(|s| s.enabled).collect();
    let section_count = sections.len().max(1);
    let slot_count = if dock.fill_available_space {
        section_count
    } else {
        section_count.max(3)
    };
    let section_gap = dock.section_gap.max(0.0);
    let section_width = (viewport.width - section_gap * sections.len().saturating_sub(1) as f32)
        .max(0.0)
        / slot_count as f32;

    let thickness = bar_thickness(dock);
    let scale = island_scale(dock);
    let island_color = island_background_color(dock);
    let mut row = Flex::row()
        .height(thickness)
        .align(Align::Center)
        .justify(dock_justify(dock.align))
        .gap(section_gap);
    if dock.fill_available_space {
        row = row.width(viewport.width);
    }
    for (index, section) in sections.iter().enumerate() {
        row = row.child(build_row_section(
            section,
            dock.fill_available_space.then_some(section_width),
            index == 0,
            index + 1 == sections.len(),
            dock.edge_gap.max(0.0),
            thickness,
            scale,
            dock.position,
            dock.show_island_background,
            dock.show_island_border,
            dock.unify_island_background,
            island_color,
            dock.island_background_opacity,
            registry,
            shared,
            open_panel,
        ));
    }

    Box::new(
        Flex::column()
            .size(viewport.width, viewport.height)
            .justify(edge_justify(dock.position))
            .align(dock_cross_align(dock.align))
            .background(Color::rgba(0, 0, 0, 0))
            .child(Box::new(dock_chrome(row, dock))),
    )
}

fn build_vertical_dock(
    viewport: Size,
    dock: &DockConfig,
    registry: &PluginRegistry,
    shared: &SharedState,
    open_panel: &Rc<dyn Fn(&'static str, Point)>,
) -> BoxedWidget {
    let sections: Vec<&SectionConfig> = dock.sections.iter().filter(|s| s.enabled).collect();
    let section_count = sections.len().max(1);
    let slot_count = if dock.fill_available_space {
        section_count
    } else {
        section_count.max(3)
    };
    let section_gap = dock.section_gap.max(0.0);
    let section_height = (viewport.height - section_gap * sections.len().saturating_sub(1) as f32)
        .max(0.0)
        / slot_count as f32;

    let thickness = bar_thickness(dock);
    let scale = island_scale(dock);
    let island_color = island_background_color(dock);
    let mut column = Flex::column()
        .width(thickness)
        .align(Align::Center)
        .justify(dock_justify(dock.align))
        .gap(section_gap);
    if dock.fill_available_space {
        column = column.height(viewport.height);
    }
    for (index, section) in sections.iter().enumerate() {
        column = column.child(build_column_section(
            section,
            dock.fill_available_space.then_some(section_height),
            index == 0,
            index + 1 == sections.len(),
            dock.edge_gap.max(0.0),
            thickness,
            scale,
            dock.position,
            dock.show_island_background,
            dock.show_island_border,
            dock.unify_island_background,
            island_color,
            dock.island_background_opacity,
            registry,
            shared,
            open_panel,
        ));
    }
    Box::new(
        Flex::row()
            .size(viewport.width, viewport.height)
            .align(dock_cross_align(dock.align))
            .justify(edge_justify(dock.position))
            .background(Color::rgba(0, 0, 0, 0))
            .child(Box::new(dock_chrome(column, dock))),
    )
}

/// One horizontal-dock section: a row of islands, `width` wide, sitting at
/// the leading, middle, or trailing edge of the dock depending on its
/// position among its siblings.
#[allow(clippy::too_many_arguments)]
fn build_row_section(
    section: &SectionConfig,
    width: Option<f32>,
    is_first: bool,
    is_last: bool,
    edge_gap: f32,
    thickness: f32,
    scale: f32,
    position: DockPosition,
    island_background: bool,
    island_border: bool,
    unify_background: bool,
    island_background_color: Option<Color>,
    island_background_opacity: f32,
    registry: &PluginRegistry,
    shared: &SharedState,
    open_panel: &Rc<dyn Fn(&'static str, Point)>,
) -> BoxedWidget {
    let justify = section_justify(is_first, is_last);
    let gap = parse_gap(&section.gap);
    let mut row = Flex::row()
        .height(thickness)
        .align(Align::Center)
        .justify(justify);
    if let Some(width) = width {
        row = row.width(width);
    }
    if is_first {
        row = row.child(Box::new(Flex::row().size(edge_gap, thickness)));
    }

    let islands = build_islands(
        section,
        position,
        scale,
        island_background && !unify_background,
        island_border && !unify_background,
        if unify_background { None } else { island_background_color },
        island_background_opacity,
        |id| registry.island(id).cloned(),
        shared,
        open_panel,
    );
    if unify_background {
        let mut inner = Flex::row().align(Align::Center).padding(4.0 * scale);
        inner = apply_gap(inner, gap, width.unwrap_or(0.0));
        for widget in islands {
            inner = inner.child(widget);
        }
        row = row.child(Box::new(section_chrome(
            inner,
            island_background,
            island_border,
            island_background_color,
            island_background_opacity,
            scale,
        )));
    } else {
        row = apply_gap(row, gap, width.unwrap_or(0.0));
        for widget in islands {
            row = row.child(widget);
        }
    }

    if is_last {
        row = row.child(Box::new(Flex::row().size(edge_gap, thickness)));
    }
    Box::new(row)
}

/// One vertical-dock section: a column of islands, `height` tall.
#[allow(clippy::too_many_arguments)]
fn build_column_section(
    section: &SectionConfig,
    height: Option<f32>,
    is_first: bool,
    is_last: bool,
    edge_gap: f32,
    thickness: f32,
    scale: f32,
    position: DockPosition,
    island_background: bool,
    island_border: bool,
    unify_background: bool,
    island_background_color: Option<Color>,
    island_background_opacity: f32,
    registry: &PluginRegistry,
    shared: &SharedState,
    open_panel: &Rc<dyn Fn(&'static str, Point)>,
) -> BoxedWidget {
    let justify = section_justify(is_first, is_last);
    let gap = parse_gap(&section.gap);
    let mut column = Flex::column()
        .width(thickness)
        .padding(8.0 * scale)
        .align(Align::Center)
        .justify(justify);
    if let Some(height) = height {
        column = column.height(height);
    }
    if is_first {
        column = column.child(Box::new(Flex::column().size(thickness, edge_gap)));
    }

    let islands = build_islands(
        section,
        position,
        scale,
        island_background && !unify_background,
        island_border && !unify_background,
        if unify_background { None } else { island_background_color },
        island_background_opacity,
        |id| registry.island(id).cloned(),
        shared,
        open_panel,
    );
    if unify_background {
        let mut inner = Flex::column().align(Align::Center).padding(4.0 * scale);
        inner = apply_gap(inner, gap, height.unwrap_or(0.0));
        for widget in islands {
            inner = inner.child(widget);
        }
        column = column.child(Box::new(section_chrome(
            inner,
            island_background,
            island_border,
            island_background_color,
            island_background_opacity,
            scale,
        )));
    } else {
        column = apply_gap(column, gap, height.unwrap_or(0.0));
        for widget in islands {
            column = column.child(widget);
        }
    }

    if is_last {
        column = column.child(Box::new(Flex::column().size(thickness, edge_gap)));
    }
    Box::new(column)
}

fn section_chrome(
    mut container: Flex,
    show_background: bool,
    show_border: bool,
    background_color: Option<Color>,
    opacity: f32,
    scale: f32,
) -> Flex {
    if show_background {
        let color = background_color.unwrap_or(Color::rgba(0, 0, 0, 0));
        container = container.background(apply_opacity(color, opacity));
    }
    if show_border {
        container = container.border(dock_border(), 1.0);
    }
    container.corner_radius(ISLAND_RADIUS * scale)
}

/// The first section hugs the dock's leading edge, the last hugs its
/// trailing edge, and anything in between centers within its own share of
/// the dock's length.
fn section_justify(is_first: bool, is_last: bool) -> Justify {
    match (is_first, is_last) {
        (true, true) => Justify::Center,
        (true, false) => Justify::Start,
        (false, true) => Justify::End,
        (false, false) => Justify::Center,
    }
}

fn dock_justify(align: DockAlign) -> Justify {
    match align {
        DockAlign::Start => Justify::Start,
        DockAlign::Center => Justify::Center,
        DockAlign::End => Justify::End,
    }
}

fn dock_cross_align(align: DockAlign) -> Align {
    match align {
        DockAlign::Start => Align::Start,
        DockAlign::Center => Align::Center,
        DockAlign::End => Align::End,
    }
}

fn edge_justify(position: DockPosition) -> Justify {
    match position {
        DockPosition::Top | DockPosition::Left => Justify::End,
        DockPosition::Bottom | DockPosition::Right => Justify::Start,
    }
}

fn dock_chrome(mut widget: Flex, dock: &DockConfig) -> Flex {
    if dock.show_background {
        widget = widget.background(dock_background(dock));
    }
    if dock.show_border {
        widget = widget.border(dock_border(), 1.0);
    }
    widget
}

fn apply_gap(container: Flex, gap: Gap, length: f32) -> Flex {
    match gap {
        Gap::Fixed(px) => container.gap(px),
        Gap::Percent(fraction) => container.gap(fraction * length),
        Gap::Between => container.gap(0.0).justify(Justify::Between),
        Gap::Evenly => container.gap(0.0).justify(Justify::Evenly),
    }
}

/// Resolves a section's configured islands against a lookup, in `index`
/// order, skipping unknown ids (already warned about at config-load time
/// via [`coconut_plugin_kit::warn_unknown_islands`], so this stays silent)
/// and islands that decline to render themselves right now (e.g.
/// "current_playing" with nothing playing).
///
/// Unlike the old `HashMap<String, BoxedWidget>` catalog this replaces,
/// `lookup` is a non-destructive read: the same id placed in two sections
/// (or twice in one section) renders twice, an intentional behavior change
/// — see `section_resolution_skips_unknown_but_allows_repeated_ids` below.
/// Takes a lookup closure rather than `&PluginRegistry` directly so this
/// can be unit-tested without a live `PluginInitContext`/`AppHandle`.
#[allow(clippy::too_many_arguments)]
fn build_islands(
    section: &SectionConfig,
    position: DockPosition,
    scale: f32,
    island_background: bool,
    island_border: bool,
    island_background_color: Option<Color>,
    island_background_opacity: f32,
    lookup: impl Fn(&str) -> Option<Rc<dyn Island>>,
    shared: &SharedState,
    open_panel: &Rc<dyn Fn(&'static str, Point)>,
) -> Vec<BoxedWidget> {
    if let Some(revision) = shared.get::<ModuleConfigRevision>() {
        revision.0.get();
    }
    let mut widgets = Vec::new();
    for entry in ordered_islands(&section.islands) {
        let Some(island) = lookup(&entry.id) else {
            continue;
        };
        let config = island_config(entry);
        let ctx = IslandRenderContext {
            shared,
            config: &config,
            position,
            scale,
            open_panel: open_panel.clone(),
        };
        if !island.is_visible(&ctx) {
            continue;
        }
        widgets.push(with_island_chrome(
            IslandChrome {
                background: island_background,
                border: island_border,
                background_color: island_background_color,
                background_opacity: island_background_opacity,
            },
            || island.build(&ctx),
        ));
    }
    widgets
}

/// Resolves an island's effective config: its `modules/<id>.toml` defaults
/// (Settings' generic per-island page writes there) with this placement's
/// inline `[[dock.section.island]].config` overrides layered on top.
/// Non-scalar values (arrays, nested tables, datetimes) aren't representable
/// as a [`ConfigValue`] and are dropped.
fn island_config(entry: &IslandEntry) -> IslandConfig {
    let mut config = module_defaults(&entry.id);
    for (key, value) in &entry.config {
        let Some(value) = toml_scalar(value) else {
            continue;
        };
        config.insert(key.clone(), value);
    }
    config
}

fn module_defaults(id: &str) -> IslandConfig {
    let toml::Value::Table(table) = coconut_core::modules::load_module_value(id) else {
        return HashMap::new();
    };
    table
        .into_iter()
        .filter_map(|(key, value)| toml_scalar(&value).map(|value| (key, value)))
        .collect()
}

fn toml_scalar(value: &toml::Value) -> Option<ConfigValue> {
    match value {
        toml::Value::Boolean(value) => Some(ConfigValue::Bool(*value)),
        toml::Value::Integer(value) => Some(ConfigValue::Int(*value)),
        toml::Value::Float(value) => Some(ConfigValue::Float(*value)),
        toml::Value::String(value) => Some(ConfigValue::String(value.clone())),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    struct StubIsland(&'static str);
    impl Island for StubIsland {
        fn id(&self) -> &'static str {
            self.0
        }
        fn build(&self, _ctx: &IslandRenderContext) -> BoxedWidget {
            Box::new(Flex::row())
        }
    }

    #[test]
    fn section_resolution_skips_unknown_but_allows_repeated_ids() {
        let mut known: HashMap<&str, Rc<dyn Island>> = HashMap::new();
        known.insert("a", Rc::new(StubIsland("a")));
        known.insert("b", Rc::new(StubIsland("b")));
        let shared = SharedState::default();
        let open_panel: Rc<dyn Fn(&'static str, Point)> = Rc::new(|_, _| {});
        let section = SectionConfig {
            islands: vec![
                IslandEntry::with_id("a"),
                IslandEntry::with_id("unknown"),
                IslandEntry::with_id("a"),
            ],
            ..Default::default()
        };
        let widgets = build_islands(
            &section,
            DockPosition::Bottom,
            1.0,
            true,
            true,
            None,
            1.0,
            |id| known.get(id).cloned(),
            &shared,
            &open_panel,
        );
        // "a" renders twice (once per entry, a non-destructive lookup);
        // "unknown" is silently skipped.
        assert_eq!(widgets.len(), 2);
    }

    #[test]
    fn section_justify_hugs_the_dock_edges() {
        assert_eq!(section_justify(true, false), Justify::Start);
        assert_eq!(section_justify(false, true), Justify::End);
        assert_eq!(section_justify(false, false), Justify::Center);
        assert_eq!(section_justify(true, true), Justify::Center);
    }
}
