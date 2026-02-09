pub mod primitives;

use glam::{Vec2, Vec3, Vec4};

use crate::scene::mesh::Face;
use crate::scene::Scene;
use crate::util::picking::{self, Ray};

/// Which draw tool is active.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrawTool {
    Tile,
    Sticky,
    Block,
    Primitive,
    VertexColor,
    Prefab,
}

/// Primitive shapes available for the Primitive draw tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrimitiveShape {
    Box,
    Cylinder,
    Cone,
    Sphere,
    Wedge,
}

/// Backup of draw state for palette override restoration.
pub struct PaletteBackup {
    pub selected_tile: (u32, u32),
    pub selected_tile_end: (u32, u32),
    pub tilebrush_rotation: u8,
    pub tilebrush_flip_h: bool,
    pub tilebrush_flip_v: bool,
    pub active_tileset: Option<usize>,
}

/// Result of a placement computation.
pub struct PlacementResult {
    pub layer: usize,
    pub object: usize,
    pub faces: Vec<Face>,
    pub create_object: bool,
    pub tileset_index: Option<usize>,
}

/// Active draw-mode state.
pub struct DrawState {
    pub tool: DrawTool,
    /// Top-left corner of selected tile region in tileset grid coords.
    pub selected_tile: (u32, u32),
    /// Bottom-right corner of selected tile region (inclusive). Same as selected_tile for single tile.
    pub selected_tile_end: (u32, u32),
    /// Selected primitive shape for the Primitive tool.
    pub selected_primitive: PrimitiveShape,
    /// Color for the Vertex Color tool.
    pub paint_color: [f32; 4],
    /// Zoom level for the tileset panel display.
    pub tileset_zoom: f32,
    /// Brush radius for vertex color painting (0 = single face).
    pub paint_radius: f32,
    /// Opacity for vertex color painting.
    pub paint_opacity: f32,
    /// Camera-derived placement plane normal (updated each frame).
    pub placement_normal: Vec3,
    /// Tilebrush rotation (0=0°, 1=90°, 2=180°, 3=270°).
    pub tilebrush_rotation: u8,
    /// Tilebrush horizontal flip.
    pub tilebrush_flip_h: bool,
    /// Tilebrush vertical flip.
    pub tilebrush_flip_v: bool,
    /// Whether the tileset panel is floating (true) or docked at bottom (false).
    pub tileset_panel_floating: bool,
    /// Block tool subtract mode: when true, block removes overlapping faces instead of adding.
    pub block_subtract: bool,
}

impl DrawState {
    pub fn new() -> Self {
        Self {
            tool: DrawTool::Tile,
            selected_tile: (0, 0),
            selected_tile_end: (0, 0),
            selected_primitive: PrimitiveShape::Box,
            paint_color: [1.0, 0.0, 0.0, 1.0],
            tileset_zoom: 1.0,
            paint_radius: 0.0,
            paint_opacity: 1.0,
            placement_normal: Vec3::Y,
            tilebrush_rotation: 0,
            tilebrush_flip_h: false,
            tilebrush_flip_v: false,
            tileset_panel_floating: false,
            block_subtract: false,
        }
    }

    /// Returns (cols, rows) of the current tile selection region.
    pub fn tile_selection_size(&self) -> (u32, u32) {
        let c0 = self.selected_tile.0.min(self.selected_tile_end.0);
        let c1 = self.selected_tile.0.max(self.selected_tile_end.0);
        let r0 = self.selected_tile.1.min(self.selected_tile_end.1);
        let r1 = self.selected_tile.1.max(self.selected_tile_end.1);
        (c1 - c0 + 1, r1 - r0 + 1)
    }

    /// Transform tile UVs according to current tilebrush rotation and flip settings.
    pub fn transform_tile_uvs(&self, mut uvs: [Vec2; 4]) -> [Vec2; 4] {
        // Apply rotation (cycle UVs clockwise)
        for _ in 0..self.tilebrush_rotation {
            uvs = [uvs[3], uvs[0], uvs[1], uvs[2]];
        }
        // Apply horizontal flip: swap left↔right
        if self.tilebrush_flip_h {
            uvs.swap(0, 1);
            uvs.swap(2, 3);
        }
        // Apply vertical flip: swap top↔bottom
        if self.tilebrush_flip_v {
            uvs.swap(0, 3);
            uvs.swap(1, 2);
        }
        uvs
    }

    /// Compute the face(s) to place and target location.
    pub fn compute_placement(
        &self,
        scene: &Scene,
        ray: &Ray,
    ) -> Option<PlacementResult> {
        match self.tool {
            DrawTool::Tile => self.compute_tile_placement(scene, ray),
            DrawTool::Sticky => self.compute_sticky_placement(scene, ray),
            DrawTool::Block => self.compute_block_placement(scene, ray),
            DrawTool::Primitive => self.compute_primitive_placement(scene, ray),
            DrawTool::VertexColor => None, // Handled separately in app.rs
            DrawTool::Prefab => self.compute_prefab_placement(scene, ray),
        }
    }

    fn compute_prefab_placement(&self, scene: &Scene, ray: &Ray) -> Option<PlacementResult> {
        let prefab = scene.active_prefab.and_then(|i| scene.prefabs.get(i))?;

        // Determine placement position: snap to grid at crosshair or hit face
        let hit = picking::pick_face_culled(ray, scene);
        let position = if let Some(ref hit) = hit {
            let offset = hit.normal * scene.grid_cell_size;
            snap_to_grid(hit.position + offset, scene.grid_cell_size)
        } else {
            let grid_normal = self.placement_normal;
            if let Some(t) = ray.intersect_plane(scene.crosshair_pos, grid_normal) {
                snap_to_grid(ray.point_at(t), scene.grid_cell_size)
            } else {
                scene.crosshair_pos
            }
        };

        let faces = prefab.instantiate_at(position);
        let ts_idx = prefab.tileset_index;
        let layer_idx = scene.active_layer;
        let (object_idx, create_object) = find_target_object(scene, layer_idx, ts_idx);

        Some(PlacementResult {
            layer: layer_idx,
            object: object_idx,
            faces,
            create_object,
            tileset_index: ts_idx,
        })
    }

    fn compute_tile_placement(&self, scene: &Scene, ray: &Ray) -> Option<PlacementResult> {
        let hit = picking::pick_face_culled(ray, scene);

        let (center, normal) = if let Some(ref hit) = hit {
            let offset = hit.normal * scene.grid_cell_size;
            (snap_to_grid(hit.position + offset, scene.grid_cell_size), hit.normal)
        } else {
            let grid_normal = self.placement_normal;
            if let Some(t) = ray.intersect_plane(scene.crosshair_pos, grid_normal) {
                let pos = ray.point_at(t);
                (snap_to_grid(pos, scene.grid_cell_size), grid_normal)
            } else {
                (scene.crosshair_pos, grid_normal)
            }
        };

        let uvs = self.tile_uvs(scene);
        let (tile_cols, tile_rows) = self.tile_selection_size();
        let face = if tile_cols == 1 && tile_rows == 1 {
            let half_size = scene.grid_cell_size * 0.5;
            Face::new_quad(center, normal, half_size, uvs)
        } else {
            let half_w = scene.grid_cell_size * tile_cols as f32 * 0.5;
            let half_h = scene.grid_cell_size * tile_rows as f32 * 0.5;
            Face::new_rect_quad(center, normal, half_w, half_h, uvs)
        };

        let layer_idx = scene.active_layer;
        let (object_idx, create_object) = find_target_object(scene, layer_idx, scene.active_tileset);

        Some(PlacementResult {
            layer: layer_idx,
            object: object_idx,
            faces: vec![face],
            create_object,
            tileset_index: scene.active_tileset,
        })
    }

    /// Sticky tool: place a tile extending from the closest edge of a hit face.
    fn compute_sticky_placement(&self, scene: &Scene, ray: &Ray) -> Option<PlacementResult> {
        let hit = picking::pick_face_culled(ray, scene)?;
        let face = &scene.layers[hit.layer_index].objects[hit.object_index].faces[hit.face_index];

        let edge_idx = closest_edge(&face.positions, hit.position);
        let a = face.positions[edge_idx];
        let b = face.positions[(edge_idx + 1) % 4];
        let face_normal = face.normal();
        let cell_size = scene.grid_cell_size;

        let new_face = Face {
            positions: [a, b, b + face_normal * cell_size, a + face_normal * cell_size],
            uvs: self.tile_uvs(scene),
            colors: [Vec4::ONE; 4],
            hidden: false,
        };

        Some(PlacementResult {
            layer: hit.layer_index,
            object: hit.object_index,
            faces: vec![new_face],
            create_object: false,
            tileset_index: scene.active_tileset,
        })
    }

    /// Block tool: place a 6-face cube at the target position.
    fn compute_block_placement(&self, scene: &Scene, ray: &Ray) -> Option<PlacementResult> {
        let hit = picking::pick_face_culled(ray, scene);

        let half = scene.grid_cell_size * 0.5;

        let center = if let Some(ref hit) = hit {
            // Use the hit face's centroid (not raw click point) for stable adjacency.
            // This ensures clicking anywhere on the same face always yields the same block position.
            let face = &scene.layers[hit.layer_index].objects[hit.object_index].faces[hit.face_index];
            let centroid = (face.positions[0] + face.positions[1] + face.positions[2] + face.positions[3]) * 0.25;
            let push = centroid + hit.normal * half;
            snap_to_cell_center(push, scene.grid_cell_size)
        } else {
            let grid_normal = self.placement_normal;
            if let Some(t) = ray.intersect_plane(scene.crosshair_pos, grid_normal) {
                let push = ray.point_at(t) + self.placement_normal * 0.01;
                snap_to_cell_center(push, scene.grid_cell_size)
            } else {
                scene.crosshair_pos
            }
        };

        let uvs = self.tile_uvs(scene);

        let faces = vec![
            Face::new_quad(center + Vec3::new(0.0, half, 0.0), Vec3::Y, half, uvs),    // top
            Face::new_quad(center - Vec3::new(0.0, half, 0.0), -Vec3::Y, half, uvs),   // bottom
            Face::new_quad(center + Vec3::new(0.0, 0.0, half), Vec3::Z, half, uvs),    // back
            Face::new_quad(center - Vec3::new(0.0, 0.0, half), -Vec3::Z, half, uvs),   // front
            Face::new_quad(center + Vec3::new(half, 0.0, 0.0), Vec3::X, half, uvs),    // right
            Face::new_quad(center - Vec3::new(half, 0.0, 0.0), -Vec3::X, half, uvs),   // left
        ];

        let layer_idx = scene.active_layer;
        let (object_idx, create_object) = find_target_object(scene, layer_idx, scene.active_tileset);

        Some(PlacementResult {
            layer: layer_idx,
            object: object_idx,
            faces,
            create_object,
            tileset_index: scene.active_tileset,
        })
    }

    /// Get UVs for the currently selected tile region from the active tileset,
    /// with tilebrush rotation/flip applied.
    /// Apply palette override: pick from active palette and set draw state temporarily.
    /// Returns the old state to restore after placement.
    pub fn apply_palette(&mut self, scene: &mut Scene) -> Option<PaletteBackup> {
        let pal_idx = scene.active_palette?;
        let palette = scene.palettes.get_mut(pal_idx)?;
        let (ts_idx, col, row, rotation, flip_h, flip_v) = palette.pick()?;

        let backup = PaletteBackup {
            selected_tile: self.selected_tile,
            selected_tile_end: self.selected_tile_end,
            tilebrush_rotation: self.tilebrush_rotation,
            tilebrush_flip_h: self.tilebrush_flip_h,
            tilebrush_flip_v: self.tilebrush_flip_v,
            active_tileset: scene.active_tileset,
        };

        self.selected_tile = (col, row);
        self.selected_tile_end = (col, row);
        self.tilebrush_rotation = rotation;
        self.tilebrush_flip_h = flip_h;
        self.tilebrush_flip_v = flip_v;
        scene.active_tileset = Some(ts_idx);

        Some(backup)
    }

    /// Restore draw state after palette placement.
    pub fn restore_palette(&mut self, scene: &mut Scene, backup: PaletteBackup) {
        self.selected_tile = backup.selected_tile;
        self.selected_tile_end = backup.selected_tile_end;
        self.tilebrush_rotation = backup.tilebrush_rotation;
        self.tilebrush_flip_h = backup.tilebrush_flip_h;
        self.tilebrush_flip_v = backup.tilebrush_flip_v;
        scene.active_tileset = backup.active_tileset;
    }

    /// Compute all tile faces for a rectangular fill between two grid positions.
    /// `start` and `end` are grid-snapped centers on the placement plane.
    /// `normal` is the face normal for all placed tiles.
    pub fn compute_rect_fill(&self, scene: &Scene, start: Vec3, end: Vec3, normal: Vec3) -> Vec<Face> {
        let cell = scene.grid_cell_size;
        let half = cell * 0.5;
        let uvs = self.tile_uvs(scene);
        let (tile_cols, tile_rows) = self.tile_selection_size();
        let step_x = cell * tile_cols as f32;
        let step_y = cell * tile_rows as f32;

        // Compute tangent basis for the normal
        let n = normal.normalize();
        let reference = if n.y.abs() > 0.9 { Vec3::Z } else { Vec3::Y };
        let right = reference.cross(n).normalize();
        let up = n.cross(right).normalize();

        // Project start and end onto the tangent plane
        let d = end - start;
        let du = d.dot(right);
        let dv = d.dot(up);

        // Number of tiles in each direction
        let count_u = ((du.abs() / step_x).round() as i32).max(0) + 1;
        let count_v = ((dv.abs() / step_y).round() as i32).max(0) + 1;

        let sign_u = if du >= 0.0 { 1.0 } else { -1.0 };
        let sign_v = if dv >= 0.0 { 1.0 } else { -1.0 };

        let mut faces = Vec::with_capacity((count_u * count_v) as usize);
        for iv in 0..count_v {
            for iu in 0..count_u {
                let center = start + right * (iu as f32 * step_x * sign_u) + up * (iv as f32 * step_y * sign_v);
                let face = if tile_cols == 1 && tile_rows == 1 {
                    Face::new_quad(center, normal, half, uvs)
                } else {
                    let half_w = cell * tile_cols as f32 * 0.5;
                    let half_h = cell * tile_rows as f32 * 0.5;
                    Face::new_rect_quad(center, normal, half_w, half_h, uvs)
                };
                faces.push(face);
            }
        }

        faces
    }

    pub fn tile_uvs(&self, scene: &Scene) -> [Vec2; 4] {
        let base_uvs = if let Some(active_idx) = scene.active_tileset
            && let Some(tileset) = scene.tilesets.get(active_idx)
        {
            let c0 = self.selected_tile.0.min(self.selected_tile_end.0);
            let c1 = self.selected_tile.0.max(self.selected_tile_end.0);
            let r0 = self.selected_tile.1.min(self.selected_tile_end.1);
            let r1 = self.selected_tile.1.max(self.selected_tile_end.1);
            tileset.tile_region_uvs(c0, r0, c1, r1)
        } else {
            default_uvs()
        };
        self.transform_tile_uvs(base_uvs)
    }

    /// Primitive tool: place a primitive shape at the target position.
    fn compute_primitive_placement(&self, scene: &Scene, ray: &Ray) -> Option<PlacementResult> {
        let hit = picking::pick_face_culled(ray, scene);

        let half = scene.grid_cell_size * 0.5;

        let center = if let Some(ref hit) = hit {
            // Use the hit face's centroid for stable adjacency (same as block tool)
            let face = &scene.layers[hit.layer_index].objects[hit.object_index].faces[hit.face_index];
            let centroid = (face.positions[0] + face.positions[1] + face.positions[2] + face.positions[3]) * 0.25;
            let push = centroid + hit.normal * half;
            snap_to_cell_center(push, scene.grid_cell_size)
        } else {
            let grid_normal = self.placement_normal;
            if let Some(t) = ray.intersect_plane(scene.crosshair_pos, grid_normal) {
                let push = ray.point_at(t) + self.placement_normal * 0.01;
                snap_to_cell_center(push, scene.grid_cell_size)
            } else {
                scene.crosshair_pos
            }
        };
        let uvs = self.tile_uvs(scene);

        let faces = match self.selected_primitive {
            PrimitiveShape::Box => primitives::generate_box(center, Vec3::splat(half), uvs),
            PrimitiveShape::Cylinder => primitives::generate_cylinder(center, half, scene.grid_cell_size, 8, uvs),
            PrimitiveShape::Cone => primitives::generate_cone(center, half, scene.grid_cell_size, 8, uvs),
            PrimitiveShape::Sphere => primitives::generate_sphere(center, half, 6, 8, uvs),
            PrimitiveShape::Wedge => primitives::generate_wedge(center, Vec3::splat(half), uvs),
        };

        let layer_idx = scene.active_layer;
        let (object_idx, create_object) = find_target_object(scene, layer_idx, scene.active_tileset);

        Some(PlacementResult {
            layer: layer_idx,
            object: object_idx,
            faces,
            create_object,
            tileset_index: scene.active_tileset,
        })
    }

    /// Compute which face to erase. Returns (layer, object, face_index, face_data).
    pub fn compute_erase(
        &self,
        scene: &Scene,
        ray: &Ray,
    ) -> Option<(usize, usize, usize, Face)> {
        if let Some(hit) = picking::pick_face_culled(ray, scene) {
            let face = scene.layers[hit.layer_index].objects[hit.object_index].faces[hit.face_index].clone();
            Some((hit.layer_index, hit.object_index, hit.face_index, face))
        } else {
            None
        }
    }
}

/// Determine the placement plane normal from the camera's forward direction.
/// The tile normal faces toward the camera. The dominant axis selects the plane.
pub fn camera_placement_normal(camera_forward: Vec3) -> Vec3 {
    let ax = camera_forward.x.abs();
    let ay = camera_forward.y.abs();
    let az = camera_forward.z.abs();
    if ay >= ax && ay >= az {
        if camera_forward.y > 0.0 { -Vec3::Y } else { Vec3::Y }
    } else if ax >= az {
        if camera_forward.x > 0.0 { -Vec3::X } else { Vec3::X }
    } else if camera_forward.z > 0.0 {
        -Vec3::Z
    } else {
        Vec3::Z
    }
}

pub fn default_uvs() -> [Vec2; 4] {
    [
        Vec2::new(0.0, 1.0),
        Vec2::new(1.0, 1.0),
        Vec2::new(1.0, 0.0),
        Vec2::new(0.0, 0.0),
    ]
}

/// Snap a world position to the nearest grid intersection (for flat tiles).
fn snap_to_grid(pos: Vec3, cell_size: f32) -> Vec3 {
    Vec3::new(
        (pos.x / cell_size).round() * cell_size,
        (pos.y / cell_size).round() * cell_size,
        (pos.z / cell_size).round() * cell_size,
    )
}

/// Snap a world position to the nearest grid cell center (for volumetric objects like blocks).
/// Grid cells span [n*cell_size, (n+1)*cell_size], so centers are at (n+0.5)*cell_size.
fn snap_to_cell_center(pos: Vec3, cell_size: f32) -> Vec3 {
    let half = cell_size * 0.5;
    Vec3::new(
        (pos.x / cell_size).floor() * cell_size + half,
        (pos.y / cell_size).floor() * cell_size + half,
        (pos.z / cell_size).floor() * cell_size + half,
    )
}

/// Find the closest edge of a quad to a point. Returns edge index (0..4).
fn closest_edge(positions: &[Vec3; 4], point: Vec3) -> usize {
    (0..4)
        .min_by(|&i, &j| {
            let mid_i = (positions[i] + positions[(i + 1) % 4]) * 0.5;
            let mid_j = (positions[j] + positions[(j + 1) % 4]) * 0.5;
            let di = mid_i.distance_squared(point);
            let dj = mid_j.distance_squared(point);
            di.partial_cmp(&dj).unwrap()
        })
        .unwrap()
}

/// Find an existing object in the layer that uses the same tileset, or signal to create a new one.
pub fn find_target_object(scene: &Scene, layer_idx: usize, tileset_idx: Option<usize>) -> (usize, bool) {
    if let Some(layer) = scene.layers.get(layer_idx) {
        for (i, obj) in layer.objects.iter().enumerate() {
            if obj.tileset_index == tileset_idx {
                return (i, false);
            }
        }
        (layer.objects.len(), true)
    } else {
        (0, true)
    }
}
