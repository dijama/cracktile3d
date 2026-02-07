mod object;
pub mod mesh;

pub use object::Object;
use glam::Vec3;
use serde::{Serialize, Deserialize};
use crate::tile::Tileset;

pub const GRID_PRESETS: &[f32] = &[0.125, 0.25, 0.5, 1.0, 2.0, 4.0];

#[derive(Serialize, Deserialize)]
pub struct Scene {
    pub layers: Vec<Layer>,
    pub crosshair_pos: Vec3,
    pub grid_cell_size: f32,
    #[serde(default = "default_grid_preset_index")]
    pub grid_preset_index: usize,
    pub active_layer: usize,
    #[serde(skip)]
    pub tilesets: Vec<Tileset>,
    #[serde(skip)]
    pub active_tileset: Option<usize>,
    /// Objects that need GPU mesh rebuild after property edits. Cleared each frame by app.
    #[serde(skip)]
    pub dirty_objects: Vec<(usize, usize)>,
}

fn default_grid_preset_index() -> usize { 3 }

#[derive(Serialize, Deserialize)]
pub struct Layer {
    pub name: String,
    pub visible: bool,
    pub objects: Vec<Object>,
}

impl Scene {
    pub fn new() -> Self {
        Self {
            layers: vec![Layer {
                name: "Layer 1".to_string(),
                visible: true,
                objects: Vec::new(),
            }],
            crosshair_pos: Vec3::ZERO,
            grid_cell_size: 1.0,
            grid_preset_index: 3,
            active_layer: 0,
            tilesets: Vec::new(),
            active_tileset: None,
            dirty_objects: Vec::new(),
        }
    }
}
