mod object;
pub mod mesh;

pub use object::{Object, GpuMesh};
use glam::Vec3;

pub struct Scene {
    pub layers: Vec<Layer>,
    pub crosshair_pos: Vec3,
    pub grid_cell_size: f32,
}

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
        }
    }

    pub fn active_layer_mut(&mut self) -> &mut Layer {
        &mut self.layers[0]
    }
}
