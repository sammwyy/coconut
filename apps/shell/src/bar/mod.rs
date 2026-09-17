use coconut_core::{
    ordered_islands, DockAlign, DockConfig, DockPosition, IslandEntry, SectionConfig,
};
use coconut_plugin_kit::{
    parse_gap, with_island_chrome, ConfigValue, Gap, Island, IslandChrome, IslandConfig,
    IslandRenderContext, PluginRegistry, SharedState,
};
use creamui_core::{BoxedWidget, Point, Size, Styled};
use creamui_theme::{use_theme, Color};
use creamui_widgets::layout::{Align, Flex, Justify};
use std::collections::HashMap;
use std::rc::Rc;

/// Thickness of a horizontal dock (its height) and of a vertical dock (its
/// width). Also read by `apps/shell/src/desktop/mod.rs` to reserve desktop
/// grid space, and by `popup_for`'s anchor-rect math in `lib.rs`.
pub const DOCK_HEIGHT: u32 = 44;
pub const DOCK_WIDTH: u32 = 52;
const BAR_HEIGHT: f32 = 44.0;

fn dock_background() -> Color {
    use_theme().colors.surface
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

    let mut row = Flex::row()
        .height(BAR_HEIGHT)
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
            dock.position,
            dock.show_island_background,
            dock.show_island_border,
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

    let mut column = Flex::column()
        .width(DOCK_WIDTH as f32)
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
            dock.position,
            dock.show_island_background,
            dock.show_island_border,
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
    position: DockPosition,
    island_background: bool,
    island_border: bool,
    registry: &PluginRegistry,
    shared: &SharedState,
    open_panel: &Rc<dyn Fn(&'static str, Point)>,
) -> BoxedWidget {
    let justify = section_justify(is_first, is_last);
    let gap = parse_gap(&section.gap);
    let mut row = Flex::row()
        .height(BAR_HEIGHT)
        .align(Align::Center)
        .justify(justify);
    if let Some(width) = width {
        row = row.width(width);
    }
    row = apply_gap(row, gap, width.unwrap_or(0.0));
    if is_first {
        row = row.child(Box::new(Flex::row().size(edge_gap, BAR_HEIGHT)));
    }
    for widget in build_islands(
        section,
        position,
        island_background,
        island_border,
        |id| registry.island(id).cloned(),
        shared,
        open_panel,
    ) {
        row = row.child(widget);
    }
    if is_last {
        row = row.child(Box::new(Flex::row().size(edge_gap, BAR_HEIGHT)));
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
    position: DockPosition,
    island_background: bool,
    island_border: bool,
    registry: &PluginRegistry,
    shared: &SharedState,
    open_panel: &Rc<dyn Fn(&'static str, Point)>,
) -> BoxedWidget {
    let justify = section_justify(is_first, is_last);
    let gap = parse_gap(&section.gap);
    let mut column = Flex::column()
        .width(DOCK_WIDTH as f32)
        .padding(8.0)
        .align(Align::Center)
        .justify(justify);
    if let Some(height) = height {
        column = column.height(height);
    }
    column = apply_gap(column, gap, height.unwrap_or(0.0));
    if is_first {
        column = column.child(Box::new(Flex::column().size(DOCK_WIDTH as f32, edge_gap)));
    }
    for widget in build_islands(
        section,
        position,
        island_background,
        island_border,
        |id| registry.island(id).cloned(),
        shared,
        open_panel,
    ) {
        column = column.child(widget);
    }
    if is_last {
        column = column.child(Box::new(Flex::column().size(DOCK_WIDTH as f32, edge_gap)));
    }
    Box::new(column)
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
        widget = widget.background(dock_background());
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
fn build_islands(
    section: &SectionConfig,
    position: DockPosition,
    island_background: bool,
    island_border: bool,
    lookup: impl Fn(&str) -> Option<Rc<dyn Island>>,
    shared: &SharedState,
    open_panel: &Rc<dyn Fn(&'static str, Point)>,
) -> Vec<BoxedWidget> {
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
            open_panel: open_panel.clone(),
        };
        if !island.is_visible(&ctx) {
            continue;
        }
        widgets.push(with_island_chrome(
            IslandChrome {
                background: island_background,
                border: island_border,
            },
            || island.build(&ctx),
        ));
    }
    widgets
}

/// Converts an island entry's inline TOML overrides into the scalar-only
/// shape islands read at render time. Non-scalar values (arrays, nested
/// tables, datetimes) aren't representable and are dropped.
fn island_config(entry: &IslandEntry) -> IslandConfig {
    let mut config: IslandConfig = HashMap::new();
    for (key, value) in &entry.config {
        let value = match value {
            toml::Value::Boolean(value) => ConfigValue::Bool(*value),
            toml::Value::Integer(value) => ConfigValue::Int(*value),
            toml::Value::Float(value) => ConfigValue::Float(*value),
            toml::Value::String(value) => ConfigValue::String(value.clone()),
            _ => continue,
        };
        config.insert(key.clone(), value);
    }
    config
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
            true,
            true,
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
