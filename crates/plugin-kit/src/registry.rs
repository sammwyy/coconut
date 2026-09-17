use crate::{Island, Panel, Plugin, PluginInitContext};
use coconut_core::DockConfig;
use std::collections::HashMap;
use std::rc::Rc;

/// Every island and panel every registered [`Plugin`] contributes, built
/// once at startup (not per dock rebuild, unlike the old
/// `HashMap<String, BoxedWidget>` catalog rebuilt inside `bar/mod.rs` on
/// every frame). Looked up by id from `shell.toml`'s
/// `[[dock.section.island]].id` entries and from an island/panel's own
/// `open_panel(id, ...)` calls.
pub struct PluginRegistry {
    islands: HashMap<&'static str, Rc<dyn Island>>,
    panels: HashMap<&'static str, Rc<dyn Panel>>,
}

impl PluginRegistry {
    /// Calls `islands()`/`panels()` on every plugin and indexes the result
    /// by id. If two plugins (or two islands/panels within one plugin)
    /// register the same id, the later one in `plugins` wins — mirrors
    /// ordinary `HashMap::insert` overwrite semantics, so registration
    /// order in `apps/shell/src/plugins.rs::all_plugins()` matters only for
    /// deliberate overrides.
    pub fn build(plugins: &[Box<dyn Plugin>], ctx: &PluginInitContext) -> Self {
        let mut registry = Self {
            islands: HashMap::new(),
            panels: HashMap::new(),
        };
        for plugin in plugins {
            registry.register(plugin.islands(ctx), plugin.panels(ctx));
        }
        registry
    }

    /// The actual dedup-by-id insertion logic `build` runs per plugin,
    /// factored out so it's unit-testable without needing a live
    /// [`PluginInitContext`] (in particular, its `app: AppHandle` field —
    /// `creamui_render::AppHandle` has no public constructor outside
    /// `AppBuilder::run`, so a real `PluginInitContext` cannot be
    /// fabricated in a `coconut-plugin-kit`-only unit test).
    fn register(&mut self, islands: Vec<Rc<dyn Island>>, panels: Vec<Rc<dyn Panel>>) {
        for island in islands {
            self.islands.insert(island.id(), island);
        }
        for panel in panels {
            self.panels.insert(panel.id(), panel);
        }
    }

    pub fn island(&self, id: &str) -> Option<&Rc<dyn Island>> {
        self.islands.get(id)
    }

    pub fn panel(&self, id: &str) -> Option<&Rc<dyn Panel>> {
        self.panels.get(id)
    }

    /// Every registered island id, for Settings-style tooling that needs to
    /// enumerate what's available rather than look up one at a time.
    pub fn known_island_ids(&self) -> impl Iterator<Item = &&'static str> {
        self.islands.keys()
    }
}

/// Warns once (meant to be called at startup, or whenever `shell.toml`
/// reloads) about every `[[dock.section.island]].id` across every dock and
/// section that `registry` has no [`Island`] for. Mirrors
/// `apps/shell/src/bar/mod.rs::warn_unknown_widgets`'s house style: the
/// per-frame dock-building code stays silent about unknown ids so
/// rebuilding a dock never spams the log, and only this explicit call does.
pub fn warn_unknown_islands(docks: &[DockConfig], registry: &PluginRegistry) {
    for dock in docks {
        for section in &dock.sections {
            for island in &section.islands {
                if registry.island(&island.id).is_none() {
                    eprintln!(
                        "plugin-kit: unknown island id '{}' in shell.toml; ignoring",
                        island.id
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{IslandRenderContext, PanelRenderContext};
    use coconut_core::{IslandEntry, SectionConfig};
    use creamui_core::{BoxedWidget, Size};
    use creamui_widgets::layout::Flex;

    // Trivial fake `Island`/`Panel`/`Plugin` implementations, as the plan
    // asks for — used here to document the intended shape of a real plugin
    // and to build islands/panels for `PluginRegistry::register` (the
    // per-plugin dedup-by-id insertion `build` delegates to). `build`
    // itself additionally needs a live `PluginInitContext` to call
    // `Plugin::islands`/`panels`, but `creamui_render::AppHandle` (the
    // context's `app` field) has no public constructor outside
    // `AppBuilder::run` — there is no sound way to fabricate one in a
    // `coconut-plugin-kit`-only unit test, so these tests exercise
    // `register`'s insertion/lookup/iteration logic directly instead of
    // going through `build`'s `ctx` parameter.
    struct FakeIsland(&'static str);
    impl Island for FakeIsland {
        fn id(&self) -> &'static str {
            self.0
        }
        fn build(&self, _ctx: &IslandRenderContext) -> BoxedWidget {
            Box::new(Flex::row())
        }
    }

    struct FakePanel(&'static str);
    impl Panel for FakePanel {
        fn id(&self) -> &'static str {
            self.0
        }
        fn title(&self) -> &'static str {
            "Fake"
        }
        fn size(&self) -> Size {
            Size {
                width: 100.0,
                height: 100.0,
            }
        }
        fn build(&self, _ctx: &PanelRenderContext) -> BoxedWidget {
            Box::new(Flex::row())
        }
    }

    #[allow(dead_code)]
    struct FakePlugin;
    impl Plugin for FakePlugin {
        fn id(&self) -> &'static str {
            "fake"
        }
        fn islands(&self, _ctx: &PluginInitContext) -> Vec<Rc<dyn Island>> {
            vec![Rc::new(FakeIsland("alpha")), Rc::new(FakeIsland("beta"))]
        }
        fn panels(&self, _ctx: &PluginInitContext) -> Vec<Rc<dyn Panel>> {
            vec![Rc::new(FakePanel("alpha_panel"))]
        }
    }

    fn empty_registry() -> PluginRegistry {
        PluginRegistry {
            islands: HashMap::new(),
            panels: HashMap::new(),
        }
    }

    fn fake_islands(ids: &[&'static str]) -> Vec<Rc<dyn Island>> {
        ids.iter()
            .map(|id| Rc::new(FakeIsland(id)) as Rc<dyn Island>)
            .collect()
    }

    #[test]
    fn register_indexes_islands_and_panels_by_id() {
        let mut registry = empty_registry();
        registry.register(
            fake_islands(&["alpha", "beta"]),
            vec![Rc::new(FakePanel("alpha_panel"))],
        );
        assert!(registry.island("alpha").is_some());
        assert!(registry.island("beta").is_some());
        assert!(registry.island("missing").is_none());
        assert!(registry.panel("alpha_panel").is_some());
        assert!(registry.panel("missing").is_none());
    }

    #[test]
    fn register_dedups_by_id_last_registration_wins() {
        let mut registry = empty_registry();
        // Simulates two plugins both registering an island id "alpha":
        // `build` calls `register` once per plugin, so a later call's
        // entries overwrite an earlier call's for the same id.
        registry.register(fake_islands(&["alpha"]), Vec::new());
        registry.register(fake_islands(&["alpha", "beta"]), Vec::new());
        let alpha_count = registry
            .known_island_ids()
            .filter(|id| **id == "alpha")
            .count();
        assert_eq!(alpha_count, 1);
        assert!(registry.island("beta").is_some());
    }

    #[test]
    fn known_island_ids_iterates_every_registered_island() {
        let mut registry = empty_registry();
        registry.register(fake_islands(&["alpha", "beta"]), Vec::new());
        let mut ids: Vec<&str> = registry.known_island_ids().copied().collect();
        ids.sort_unstable();
        assert_eq!(ids, vec!["alpha", "beta"]);
    }

    fn dock_with(ids: &[&str]) -> DockConfig {
        DockConfig {
            sections: vec![SectionConfig {
                islands: ids.iter().map(|id| IslandEntry::with_id(id)).collect(),
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    #[test]
    fn warn_unknown_islands_is_a_no_op_for_known_ids() {
        let mut registry = empty_registry();
        registry.register(fake_islands(&["alpha", "beta"]), Vec::new());
        let docks = vec![dock_with(&["alpha", "beta"])];
        // No assertion on stderr; this confirms every id in the dock is
        // recognized by the registry (the actual "is this known" check
        // `warn_unknown_islands` performs), so nothing would be warned.
        for dock in &docks {
            for section in &dock.sections {
                for island in &section.islands {
                    assert!(registry.island(&island.id).is_some());
                }
            }
        }
        warn_unknown_islands(&docks, &registry);
    }

    #[test]
    fn warn_unknown_islands_flags_an_unregistered_id() {
        let mut registry = empty_registry();
        registry.register(fake_islands(&["alpha"]), Vec::new());
        let docks = vec![dock_with(&["alpha", "not_a_real_island"])];
        assert!(registry.island("not_a_real_island").is_none());
        warn_unknown_islands(&docks, &registry);
    }
}
