//! Tile palette system for randomized/sequenced tile placement.

use serde::{Serialize, Deserialize};

/// A single tile entry in a palette.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PaletteEntry {
    /// Tileset index.
    pub tileset_index: usize,
    /// Column in the tileset grid.
    pub col: u32,
    /// Row in the tileset grid.
    pub row: u32,
    /// Weight for random selection (higher = more likely).
    pub weight: f32,
}

/// How the palette selects tiles.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PaletteMode {
    /// Weighted random tile selection on each placement.
    #[default]
    Random,
    /// Cycle through tiles in order.
    Sequence,
}

/// A palette: a weighted collection of tile entries.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Palette {
    pub name: String,
    pub entries: Vec<PaletteEntry>,
    pub mode: PaletteMode,
    /// Whether to randomize tilebrush rotation per placement.
    pub random_rotation: bool,
    /// Whether to randomize horizontal flip per placement.
    pub random_flip_h: bool,
    /// Whether to randomize vertical flip per placement.
    pub random_flip_v: bool,
    /// Current index for sequence mode (not serialized).
    #[serde(skip)]
    pub sequence_index: usize,
    /// Simple RNG state (not serialized).
    #[serde(skip)]
    rng_state: u64,
}

impl Palette {
    pub fn new(name: String) -> Self {
        Self {
            name,
            entries: Vec::new(),
            mode: PaletteMode::Random,
            random_rotation: false,
            random_flip_h: false,
            random_flip_v: false,
            sequence_index: 0,
            rng_state: 12345,
        }
    }

    /// Add a tile entry with default weight 1.0.
    pub fn add_entry(&mut self, tileset_index: usize, col: u32, row: u32) {
        self.entries.push(PaletteEntry {
            tileset_index,
            col,
            row,
            weight: 1.0,
        });
    }

    /// Pick the next tile entry based on palette mode.
    /// Returns (tileset_index, col, row, rotation, flip_h, flip_v) or None if empty.
    pub fn pick(&mut self) -> Option<(usize, u32, u32, u8, bool, bool)> {
        if self.entries.is_empty() {
            return None;
        }

        let entry = match self.mode {
            PaletteMode::Random => {
                // Weighted random selection using xorshift
                let total_weight: f32 = self.entries.iter().map(|e| e.weight).sum();
                if total_weight <= 0.0 {
                    &self.entries[0]
                } else {
                    let r = self.next_random_f32() * total_weight;
                    let mut accum = 0.0;
                    let mut chosen = &self.entries[0];
                    for entry in &self.entries {
                        accum += entry.weight;
                        if r <= accum {
                            chosen = entry;
                            break;
                        }
                    }
                    chosen
                }
            }
            PaletteMode::Sequence => {
                let idx = self.sequence_index % self.entries.len();
                self.sequence_index += 1;
                &self.entries[idx]
            }
        };

        // Copy entry data before mutable borrow for RNG
        let (ts_idx, col, row) = (entry.tileset_index, entry.col, entry.row);

        let rotation = if self.random_rotation {
            (self.next_random() % 4) as u8
        } else {
            0
        };
        let flip_h = self.random_flip_h && self.next_random().is_multiple_of(2);
        let flip_v = self.random_flip_v && self.next_random().is_multiple_of(2);

        Some((ts_idx, col, row, rotation, flip_h, flip_v))
    }

    /// Normalize weights so they sum to 1.0.
    pub fn normalize_weights(&mut self) {
        let total: f32 = self.entries.iter().map(|e| e.weight).sum();
        if total > 0.0 {
            for entry in &mut self.entries {
                entry.weight /= total;
            }
        }
    }

    /// Simple xorshift64 PRNG — returns a u64.
    fn next_random(&mut self) -> u64 {
        let mut x = self.rng_state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.rng_state = x;
        x
    }

    /// Random f32 in [0, 1).
    fn next_random_f32(&mut self) -> f32 {
        (self.next_random() % 10000) as f32 / 10000.0
    }
}
