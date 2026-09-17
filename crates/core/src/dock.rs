use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DockPosition {
    Top,
    Bottom,
    Left,
    Right,
}

impl DockPosition {
    pub fn is_vertical(self) -> bool {
        matches!(self, Self::Left | Self::Right)
    }
}

impl Default for DockPosition {
    fn default() -> Self {
        DockPosition::Bottom
    }
}

/// How a dock's sections are laid out. Defaults to the axis implied by
/// `DockPosition` (row for top/bottom, column for left/right) when unset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DockDirection {
    Row,
    Column,
}

/// Where a dock's section group sits along its main axis when it is not
/// configured to fill the available space.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DockAlign {
    Start,
    Center,
    End,
}

impl Default for DockAlign {
    fn default() -> Self {
        DockAlign::Start
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DockConfig {
    /// Hides this dock (no window is created for it) without deleting its
    /// configured sections/islands — flip back on to bring it back exactly
    /// as it was.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// A human-readable label for this dock in Settings (e.g. "Dockbar").
    /// `None` falls back to a positional label ("Dock 2", ...) there —
    /// purely cosmetic, never read by the rendering engine.
    pub name: Option<String>,
    pub position: DockPosition,
    pub direction: Option<DockDirection>,
    pub align: DockAlign,
    /// Empty space at each end of the dock, in logical pixels.
    pub edge_gap: f32,
    /// Empty space between consecutive sections, in logical pixels.
    pub section_gap: f32,
    /// When true, sections divide the entire dock length. When false, the
    /// section group uses up to three equal slots and follows [`Self::align`].
    #[serde(default = "default_true")]
    pub fill_available_space: bool,
    /// Maximum dock length in logical pixels. `None` leaves the panel at the
    /// compositor-provided length.
    pub max_length: Option<f32>,
    /// Space between the dock and the screen edge it is anchored to.
    pub margin: f32,
    #[serde(default = "default_true")]
    pub show_background: bool,
    #[serde(default = "default_true")]
    pub show_border: bool,
    #[serde(default = "default_true")]
    pub show_island_background: bool,
    #[serde(default = "default_true")]
    pub show_island_border: bool,
    #[serde(rename = "section")]
    pub sections: Vec<SectionConfig>,
}

fn default_true() -> bool {
    true
}

impl DockConfig {
    /// The effective layout direction: `direction` if set, otherwise derived
    /// from `position` (a left/right dock lays its sections out as a column).
    pub fn effective_direction(&self) -> DockDirection {
        self.direction.unwrap_or(if self.position.is_vertical() {
            DockDirection::Column
        } else {
            DockDirection::Row
        })
    }
}

impl Default for DockConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            name: Some("Dockbar".to_owned()),
            position: DockPosition::default(),
            direction: None,
            align: DockAlign::default(),
            edge_gap: 16.0,
            section_gap: 0.0,
            fill_available_space: true,
            max_length: None,
            margin: 0.0,
            show_background: true,
            show_border: true,
            show_island_background: true,
            show_island_border: true,
            sections: vec![
                SectionConfig {
                    enabled: true,
                    gap: default_gap(),
                    islands: vec![
                        IslandEntry::with_id("logo"),
                        IslandEntry::with_id("weather"),
                        IslandEntry::with_id("current_playing"),
                    ],
                },
                SectionConfig {
                    enabled: true,
                    gap: default_gap(),
                    islands: vec![IslandEntry::with_id("app_launcher")],
                },
                SectionConfig {
                    enabled: true,
                    gap: default_gap(),
                    islands: vec![
                        IslandEntry::with_id("control_center"),
                        IslandEntry::with_id("clock"),
                    ],
                },
            ],
        }
    }
}

/// A pure layout region within a dock. Sections render nothing themselves;
/// with N sections in a dock, each tiles roughly `100/N`% of the dock's
/// length (minus gaps), so sections always cover the full length.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SectionConfig {
    /// Hides this section (and every island in it) without deleting its
    /// islands — flip back on to bring them back exactly as configured.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Spacing between the islands inside this section: a fixed length
    /// ("10px"), a percentage of the section's length ("30%"), or one of the
    /// keywords "between"/"evenly" (CSS-flexbox-style distribution). Parsed
    /// by `coconut-plugin-kit`.
    pub gap: String,
    #[serde(rename = "island")]
    pub islands: Vec<IslandEntry>,
}

impl Default for SectionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            gap: default_gap(),
            islands: Vec::new(),
        }
    }
}

fn default_gap() -> String {
    "10px".to_owned()
}

/// One island placed in a section. `id` must match a registered island
/// implementation; unknown ids are skipped with a warning. `index` orders
/// islands explicitly — when omitted, or when two entries share the same
/// index, position in this array is used as a stable fallback.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct IslandEntry {
    pub id: String,
    pub index: Option<u32>,
    #[serde(skip_serializing_if = "toml::value::Table::is_empty")]
    pub config: toml::value::Table,
}

impl IslandEntry {
    pub fn with_id(id: &str) -> Self {
        Self {
            id: id.to_owned(),
            index: None,
            config: toml::value::Table::new(),
        }
    }
}

impl Default for IslandEntry {
    fn default() -> Self {
        Self {
            id: String::new(),
            index: None,
            config: toml::value::Table::new(),
        }
    }
}

/// Resolves the render order of `islands`: sorts by explicit `index` when
/// present, falling back to original array position for entries with no
/// index (or a duplicate one) — stable, so ties never reorder relative to
/// each other.
pub fn ordered_islands(islands: &[IslandEntry]) -> Vec<&IslandEntry> {
    let mut ordered: Vec<(usize, &IslandEntry)> = islands.iter().enumerate().collect();
    ordered.sort_by_key(|(position, entry)| (entry.index.unwrap_or(*position as u32), *position));
    ordered.into_iter().map(|(_, entry)| entry).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordered_islands_falls_back_to_array_position() {
        let islands = vec![
            IslandEntry {
                id: "a".into(),
                index: None,
                config: Default::default(),
            },
            IslandEntry {
                id: "b".into(),
                index: Some(0),
                config: Default::default(),
            },
            IslandEntry {
                id: "c".into(),
                index: None,
                config: Default::default(),
            },
        ];
        let ids: Vec<&str> = ordered_islands(&islands)
            .into_iter()
            .map(|entry| entry.id.as_str())
            .collect();
        // "a" (position 0, no index) and "b" (explicit index 0) tie at
        // effective index 0; the tie is broken by original array position,
        // so "a" (position 0) sorts before "b" (position 1).
        assert_eq!(ids, vec!["a", "b", "c"]);
    }

    #[test]
    fn ordered_islands_duplicate_index_falls_back_to_position() {
        let islands = vec![
            IslandEntry {
                id: "first".into(),
                index: Some(5),
                config: Default::default(),
            },
            IslandEntry {
                id: "second".into(),
                index: Some(5),
                config: Default::default(),
            },
        ];
        let ids: Vec<&str> = ordered_islands(&islands)
            .into_iter()
            .map(|entry| entry.id.as_str())
            .collect();
        assert_eq!(ids, vec!["first", "second"]);
    }

    #[test]
    fn default_dock_matches_legacy_three_section_layout() {
        let dock = DockConfig::default();
        assert_eq!(dock.position, DockPosition::Bottom);
        assert_eq!(dock.sections.len(), 3);
        fn ids(section: &SectionConfig) -> Vec<&str> {
            section.islands.iter().map(|i| i.id.as_str()).collect()
        }
        assert_eq!(
            ids(&dock.sections[0]),
            vec!["logo", "weather", "current_playing"]
        );
        assert_eq!(ids(&dock.sections[1]), vec!["app_launcher"]);
        assert_eq!(ids(&dock.sections[2]), vec!["control_center", "clock"]);
    }
}
