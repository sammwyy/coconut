use serde::{Deserialize, Serialize};

pub const COLUMNS: u8 = 4;
pub(crate) const MAX_ROW: u16 = i16::MAX as u16 - 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Control {
    Wifi,
    Bluetooth,
    Battery,
    KeepAwake,
    Brightness,
    Volume,
}

impl Control {
    pub const ALL: [Self; 6] = [
        Self::Wifi,
        Self::Bluetooth,
        Self::Battery,
        Self::KeepAwake,
        Self::Brightness,
        Self::Volume,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Wifi => "Wi-Fi",
            Self::Bluetooth => "Bluetooth",
            Self::Battery => "Battery",
            Self::KeepAwake => "Keep awake",
            Self::Brightness => "Brightness",
            Self::Volume => "Volume",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ControlTile {
    pub control: Control,
    pub column: u8,
    pub row: u16,
    pub width: u8,
    pub height: u8,
}

impl ControlTile {
    fn overlaps(self, other: Self) -> bool {
        self.column < other.column + other.width
            && other.column < self.column + self.width
            && self.row < other.row + u16::from(other.height)
            && other.row < self.row + u16::from(self.height)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ControlLayout {
    pub tiles: Vec<ControlTile>,
}

impl Default for ControlLayout {
    fn default() -> Self {
        Self {
            tiles: Control::ALL
                .into_iter()
                .zip([
                    (0, 0, 2, 1),
                    (2, 0, 2, 1),
                    (2, 1, 2, 1),
                    (2, 2, 2, 1),
                    (0, 1, 1, 2),
                    (1, 1, 1, 2),
                ])
                .map(|(control, (column, row, width, height))| ControlTile {
                    control,
                    column,
                    row,
                    width,
                    height,
                })
                .collect(),
        }
    }
}

impl ControlLayout {
    pub fn normalize(&mut self) {
        let mut placed: Vec<ControlTile> = Vec::new();
        for mut tile in self.tiles.drain(..) {
            if placed.iter().any(|other| other.control == tile.control) {
                continue;
            }
            tile.width = tile.width.clamp(1, COLUMNS);
            tile.height = tile.height.clamp(1, 3);
            tile.column = tile.column.min(COLUMNS - tile.width);
            tile.row = tile.row.min(MAX_ROW);
            while placed.iter().any(|other| tile.overlaps(*other)) {
                tile.row += 1;
            }
            placed.push(tile);
        }
        self.tiles = placed;
    }

    pub fn place(&mut self, tile: ControlTile) {
        self.tiles.retain(|other| other.control != tile.control);
        self.tiles.insert(0, tile);
        self.normalize();
    }

    pub fn add(&mut self, control: Control) {
        if self.tiles.iter().any(|tile| tile.control == control) {
            return;
        }
        let mut tile = ControlTile {
            control,
            column: 0,
            row: 0,
            width: 1,
            height: 1,
        };
        while self.tiles.iter().any(|other| tile.overlaps(*other)) {
            tile.column += 1;
            if tile.column == COLUMNS {
                tile.column = 0;
                tile.row += 1;
            }
        }
        self.tiles.push(tile);
    }

    pub fn rows(&self) -> u16 {
        self.tiles
            .iter()
            .map(|tile| tile.row + u16::from(tile.height))
            .max()
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_valid(layout: &ControlLayout) {
        for (index, tile) in layout.tiles.iter().enumerate() {
            assert!((1..=COLUMNS).contains(&tile.width));
            assert!((1..=3).contains(&tile.height));
            assert!(tile.column + tile.width <= COLUMNS);
            for other in &layout.tiles[index + 1..] {
                assert_ne!(tile.control, other.control);
                assert!(!tile.overlaps(*other));
            }
        }
    }

    #[test]
    fn default_layout_places_two_medium_radios_above_vertical_levels() {
        let layout = ControlLayout::default();
        assert_valid(&layout);
        assert_eq!(COLUMNS, 4);
        assert_eq!(
            (
                layout.tiles[0].column,
                layout.tiles[0].row,
                layout.tiles[0].width
            ),
            (0, 0, 2)
        );
        assert_eq!(
            (
                layout.tiles[1].column,
                layout.tiles[1].row,
                layout.tiles[1].width
            ),
            (2, 0, 2)
        );
        for control in [Control::Brightness, Control::Volume] {
            let tile = layout
                .tiles
                .iter()
                .find(|tile| tile.control == control)
                .unwrap();
            assert_eq!((tile.width, tile.height), (1, 2));
        }
    }

    #[test]
    fn moves_and_resizes_displace_neighbors_without_overlapping() {
        let mut layout = ControlLayout::default();
        for control in Control::ALL {
            for width in 1..=COLUMNS {
                for height in 1..=3 {
                    layout.place(ControlTile {
                        control,
                        column: 2,
                        row: 0,
                        width,
                        height,
                    });
                    assert_valid(&layout);
                    assert_eq!(layout.tiles.len(), 6);
                    assert_eq!(layout.tiles[0].row, 0);
                }
            }
        }
    }

    #[test]
    fn malformed_geometry_and_duplicates_are_normalized() {
        let mut layout = ControlLayout::default();
        layout.tiles[0].width = 255;
        layout.tiles[0].height = 0;
        layout.tiles[0].column = 255;
        layout.tiles[0].row = u16::MAX;
        layout.tiles.push(layout.tiles[0]);
        layout.normalize();
        assert_valid(&layout);
        assert_eq!(layout.tiles.len(), 6);
    }

    #[test]
    fn empty_layout_stays_empty_and_controls_can_be_added_again() {
        let mut layout = ControlLayout { tiles: vec![] };
        layout.normalize();
        assert_eq!(layout.rows(), 0);
        for control in Control::ALL.into_iter().chain(Control::ALL) {
            layout.add(control);
        }
        assert_valid(&layout);
        assert_eq!(layout.tiles.len(), 6);
        assert_eq!(layout.rows(), 2);
    }

    #[test]
    fn serialization_preserves_positions_sizes_and_removals() {
        let mut layout = ControlLayout::default();
        layout.tiles.remove(1);
        layout.tiles[0].height = 3;
        layout.normalize();
        let encoded = toml::to_string(&layout).unwrap();
        assert_eq!(toml::from_str::<ControlLayout>(&encoded).unwrap(), layout);
    }
}
