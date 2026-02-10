mod object;
pub mod mesh;

pub use object::{Object, Instance};
use glam::Vec3;
use serde::{Serialize, Deserialize};
use crate::tile::Tileset;
use crate::scene::mesh::Face;
use crate::bones::Skeleton;
use crate::tile::palette::Palette;

pub const GRID_PRESETS: &[f32] = &[0.125, 0.25, 0.5, 1.0, 2.0, 4.0];

/// A reusable prefab template (geometry + metadata).
#[derive(Clone, Serialize, Deserialize)]
pub struct Prefab {
    pub name: String,
    pub faces: Vec<Face>,
    /// Offset from prefab origin (centroid computed at creation).
    pub origin: Vec3,
    /// Tileset index this prefab was created from.
    pub tileset_index: Option<usize>,
}

impl Prefab {
    /// Create a prefab from a set of faces. Computes centroid as origin.
    pub fn from_faces(name: String, faces: Vec<Face>, tileset_index: Option<usize>) -> Self {
        let origin = if faces.is_empty() {
            Vec3::ZERO
        } else {
            let sum: Vec3 = faces.iter()
                .flat_map(|f| f.positions.iter())
                .copied()
                .sum();
            let count = (faces.len() * 4) as f32;
            sum / count
        };
        Self { name, faces, origin, tileset_index }
    }

    /// Return faces translated so origin is at `position`.
    pub fn instantiate_at(&self, position: Vec3) -> Vec<Face> {
        let offset = position - self.origin;
        self.faces.iter().map(|f| {
            let mut nf = f.clone();
            for p in &mut nf.positions {
                *p += offset;
            }
            nf
        }).collect()
    }
}

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
    /// Prefab library — reusable geometry templates.
    #[serde(default)]
    pub prefabs: Vec<Prefab>,
    /// Currently selected prefab for placement.
    #[serde(skip)]
    pub active_prefab: Option<usize>,
    /// Skeleton for bone-based animation.
    #[serde(default)]
    pub skeleton: Skeleton,
    /// Tile palettes for randomized/sequenced placement.
    #[serde(default)]
    pub palettes: Vec<Palette>,
    /// Active palette index (None = direct tile selection).
    #[serde(skip)]
    pub active_palette: Option<usize>,
}

fn default_grid_preset_index() -> usize { 3 }

#[derive(Serialize, Deserialize)]
pub struct Layer {
    pub name: String,
    pub visible: bool,
    pub objects: Vec<Object>,
}

impl Scene {
    /// Count total instances and objects that have instances.
    pub fn instance_count(&self) -> (usize, usize) {
        let mut total = 0;
        let mut objects_with = 0;
        for layer in &self.layers {
            for obj in &layer.objects {
                if !obj.instances.is_empty() {
                    total += obj.instances.len();
                    objects_with += 1;
                }
            }
        }
        (total, objects_with)
    }

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
            prefabs: Vec::new(),
            active_prefab: None,
            skeleton: Skeleton::new(),
            palettes: Vec::new(),
            active_palette: None,
        }
    }
}
