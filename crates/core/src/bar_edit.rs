use crate::{DockConfig, IslandEntry, SectionConfig, ShellConfig};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BarKind {
    Statusbar,
    Dockbar,
}

impl BarKind {
    pub fn title(self) -> &'static str {
        match self {
            Self::Statusbar => "Statusbar",
            Self::Dockbar => "Dockbar",
        }
    }

    pub fn route(self) -> &'static str {
        match self {
            Self::Statusbar => "/desktop/statusbar?tab=contents",
            Self::Dockbar => "/desktop/dock?tab=contents",
        }
    }

    pub fn get(self, config: &ShellConfig) -> &DockConfig {
        match self {
            Self::Statusbar => &config.statusbar,
            Self::Dockbar => &config.dockbar,
        }
    }

    pub fn get_mut(self, config: &mut ShellConfig) -> &mut DockConfig {
        match self {
            Self::Statusbar => &mut config.statusbar,
            Self::Dockbar => &mut config.dockbar,
        }
    }
}

#[derive(Clone, Debug)]
pub enum BarEdit {
    AddGroup,
    RemoveGroup(usize),
    MoveGroup {
        from: usize,
        to: usize,
    },
    ShowGroup {
        group: usize,
        enabled: bool,
    },
    GroupGap {
        group: usize,
        gap: String,
    },
    AddItem {
        group: usize,
        at: usize,
        id: String,
    },
    RemoveItem {
        group: usize,
        item: usize,
    },
    MoveItem {
        group: usize,
        item: usize,
        to_group: usize,
        at: usize,
    },
    Restore(DockConfig),
}

impl BarEdit {
    pub fn apply_checked(
        &self,
        kind: BarKind,
        expected: &DockConfig,
        config: &mut ShellConfig,
    ) -> Result<(), String> {
        if kind.get(config) != expected {
            return Err("The bar was changed elsewhere. Try again with the latest layout.".into());
        }
        self.apply(kind.get_mut(config))
    }

    pub fn apply(&self, bar: &mut DockConfig) -> Result<(), String> {
        let mut next = bar.clone();
        self.apply_inner(&mut next)?;
        *bar = next;
        Ok(())
    }

    fn apply_inner(&self, bar: &mut DockConfig) -> Result<(), String> {
        let invalid = || "This content has changed. Reopen its menu and try again.".to_owned();
        match self {
            Self::AddGroup => bar.sections.push(SectionConfig::default()),
            Self::RemoveGroup(group) => {
                if *group >= bar.sections.len() {
                    return Err(invalid());
                }
                bar.sections.remove(*group);
            }
            Self::MoveGroup { from, to } => {
                if *from >= bar.sections.len() || *to >= bar.sections.len() {
                    return Err(invalid());
                }
                let group = bar.sections.remove(*from);
                bar.sections.insert(*to, group);
            }
            Self::ShowGroup { group, enabled } => {
                bar.sections.get_mut(*group).ok_or_else(invalid)?.enabled = *enabled;
            }
            Self::GroupGap { group, gap } => {
                bar.sections.get_mut(*group).ok_or_else(invalid)?.gap = gap.clone();
            }
            Self::AddItem { group, at, id } => {
                let section = bar.sections.get_mut(*group).ok_or_else(invalid)?;
                if *at > section.islands.len() {
                    return Err(invalid());
                }
                normalize(section);
                section.islands.insert(*at, IslandEntry::with_id(id));
            }
            Self::RemoveItem { group, item } => {
                let section = bar.sections.get_mut(*group).ok_or_else(invalid)?;
                if *item >= section.islands.len() {
                    return Err(invalid());
                }
                normalize(section);
                section.islands.remove(*item);
            }
            Self::MoveItem {
                group,
                item,
                to_group,
                at,
            } => {
                if *group >= bar.sections.len()
                    || *to_group >= bar.sections.len()
                    || *item >= bar.sections[*group].islands.len()
                    || *at > bar.sections[*to_group].islands.len()
                {
                    return Err(invalid());
                }
                normalize(&mut bar.sections[*group]);
                if group != to_group {
                    normalize(&mut bar.sections[*to_group]);
                }
                let entry = bar.sections[*group].islands.remove(*item);
                let at = if group == to_group && at > item {
                    at - 1
                } else {
                    *at
                };
                bar.sections[*to_group].islands.insert(at, entry);
            }
            Self::Restore(previous) => *bar = previous.clone(),
        }
        Ok(())
    }
}

fn normalize(section: &mut SectionConfig) {
    let mut entries: Vec<_> = std::mem::take(&mut section.islands)
        .into_iter()
        .enumerate()
        .collect();
    entries.sort_by_key(|(position, entry)| (entry.index.unwrap_or(*position as u32), *position));
    section.islands = entries
        .into_iter()
        .map(|(_, mut entry)| {
            entry.index = None;
            entry
        })
        .collect();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ordered_islands;

    #[test]
    fn stale_menus_cannot_remove_a_different_placement_or_overwrite_other_settings() {
        let mut config = ShellConfig::default();
        let expected = config.statusbar.clone();
        config.dockbar.thickness = 88.0;
        BarEdit::AddGroup
            .apply_checked(BarKind::Statusbar, &expected, &mut config)
            .unwrap();
        assert_eq!(config.dockbar.thickness, 88.0);
        let before = config.clone();
        assert!(BarEdit::RemoveItem { group: 0, item: 0 }
            .apply_checked(BarKind::Statusbar, &expected, &mut config)
            .is_err());
        assert_eq!(config, before);
    }

    #[test]
    fn editing_uses_visible_order_and_preserves_instance_settings() {
        let mut bar = DockConfig::default();
        bar.sections[0].islands[0].index = Some(9);
        bar.sections[0].islands[0]
            .config
            .insert("label".into(), "Personal launcher".into());
        BarEdit::AddGroup.apply(&mut bar).unwrap();
        BarEdit::MoveItem {
            group: 0,
            item: 1,
            to_group: 1,
            at: 0,
        }
        .apply(&mut bar)
        .unwrap();
        assert_eq!(bar.sections[0].islands[0].id, "open_windows");
        assert_eq!(bar.sections[1].islands[0].id, "app_launcher");
        assert_eq!(
            bar.sections[1].islands[0].config["label"].as_str(),
            Some("Personal launcher")
        );
        BarEdit::AddItem {
            group: 0,
            at: 0,
            id: "clipboard".into(),
        }
        .apply(&mut bar)
        .unwrap();
        assert_eq!(ordered_islands(&bar.sections[0].islands)[0].id, "clipboard");
    }

    #[test]
    fn moves_before_and_after_and_allows_duplicates() {
        let mut bar = DockConfig::default();
        BarEdit::AddItem {
            group: 0,
            at: 2,
            id: "app_launcher".into(),
        }
        .apply(&mut bar)
        .unwrap();
        BarEdit::MoveItem {
            group: 0,
            item: 0,
            to_group: 0,
            at: 3,
        }
        .apply(&mut bar)
        .unwrap();
        assert_eq!(
            bar.sections[0]
                .islands
                .iter()
                .map(|e| e.id.as_str())
                .collect::<Vec<_>>(),
            ["open_windows", "app_launcher", "app_launcher"]
        );
        let before = bar.clone();
        assert!(BarEdit::MoveItem {
            group: 0,
            item: 0,
            to_group: 99,
            at: 0
        }
        .apply(&mut bar)
        .is_err());
        assert_eq!(bar, before);
    }

    #[test]
    fn groups_can_be_hidden_reordered_removed_and_restored() {
        let mut bar = ShellConfig::default().statusbar;
        let original = bar.clone();
        BarEdit::ShowGroup {
            group: 0,
            enabled: false,
        }
        .apply(&mut bar)
        .unwrap();
        BarEdit::MoveGroup { from: 0, to: 2 }
            .apply(&mut bar)
            .unwrap();
        assert!(!bar.sections[2].enabled);
        BarEdit::RemoveGroup(2).apply(&mut bar).unwrap();
        BarEdit::Restore(original.clone()).apply(&mut bar).unwrap();
        assert_eq!(bar, original);
    }
}
