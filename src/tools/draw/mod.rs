use glam::{Vec2, Vec3, Vec4};

use crate::scene::mesh::Face;
use crate::scene::{Object, Scene};
use crate::util::picking::{self, Ray};

/// Which draw tool is active.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrawTool {
    Tile,
    Sticky,
    Block,
    Primitive,
    VertexColor,
}

/// Active draw-mode state.
pub struct DrawState {
    pub tool: DrawTool,
    pub erasing: bool,
    /// Currently selected tile region in tileset grid coords.
    pub selected_tile: (u32, u32),
}

impl DrawState {
    pub fn new() -> Self {
        Self {
            tool: DrawTool::Tile,
            erasing: false,
            selected_tile: (0, 0),
        }
    }

    /// Handle a left-click in Draw mode. Places a tile at the crosshair position
    /// on the placement plane, or adjacent to a hit face.
    pub fn place_tile(
        &self,
        scene: &mut Scene,
        ray: &Ray,
        device: &wgpu::Device,
    ) {
        if self.erasing {
            self.erase_tile(scene, ray, device);
            return;
        }

        // Try to hit existing geometry first
        let hit = picking::pick_face(ray, scene);

        let (center, normal) = if let Some(ref hit) = hit {
            // Place adjacent to the hit face, offset by one grid unit along the normal
            let offset = hit.normal * scene.grid_cell_size;
            (hit.position + offset, hit.normal)
        } else {
            // No hit — place on the XZ grid plane at the crosshair position
            let grid_normal = Vec3::Y;
            if let Some(t) = ray.intersect_plane(scene.crosshair_pos, grid_normal) {
                let pos = ray.point_at(t);
                // Snap to grid
                let snapped = snap_to_grid(pos, scene.grid_cell_size);
                (snapped, grid_normal)
            } else {
                // Fallback: place at crosshair position
                (scene.crosshair_pos, grid_normal)
            }
        };

        // Default UVs (full tile, will be replaced when tileset selection is wired)
        let uvs = [
            Vec2::new(0.0, 1.0),
            Vec2::new(1.0, 1.0),
            Vec2::new(1.0, 0.0),
            Vec2::new(0.0, 0.0),
        ];

        let half_size = scene.grid_cell_size * 0.5;
        let face = Face::new_quad(center, normal, half_size, uvs);

        // Add to the first layer's first object, or create one
        let layer = &mut scene.layers[0];
        if layer.objects.is_empty() {
            layer.objects.push(Object::new("Object 1".to_string()));
        }
        let object = &mut layer.objects[0];
        object.faces.push(face);
        object.rebuild_gpu_mesh(device);
    }

    fn erase_tile(
        &self,
        scene: &mut Scene,
        ray: &Ray,
        device: &wgpu::Device,
    ) {
        if let Some(hit) = picking::pick_face(ray, scene) {
            let object = &mut scene.layers[hit.layer_index].objects[hit.object_index];
            object.faces.remove(hit.face_index);
            object.rebuild_gpu_mesh(device);
        }
    }
}

/// Snap a world position to the nearest grid cell center.
fn snap_to_grid(pos: Vec3, cell_size: f32) -> Vec3 {
    Vec3::new(
        (pos.x / cell_size).round() * cell_size,
        (pos.y / cell_size).round() * cell_size,
        (pos.z / cell_size).round() * cell_size,
    )
}
