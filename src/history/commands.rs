use glam::{Quat, Vec2, Vec3, Vec4};
use crate::history::Command;
use crate::scene::mesh::Face;
use crate::scene::{Object, Scene};
use crate::tools::draw::default_uvs;

/// Hide selected faces (undoable).
pub struct HideFaces {
    pub faces: Vec<(usize, usize, usize)>,
}

impl Command for HideFaces {
    fn apply(&mut self, scene: &mut Scene, device: &wgpu::Device) {
        let mut rebuild: std::collections::HashSet<(usize, usize)> = std::collections::HashSet::new();
        for &(li, oi, fi) in &self.faces {
            if let Some(face) = scene.layers.get_mut(li)
                .and_then(|l| l.objects.get_mut(oi))
                .and_then(|o| o.faces.get_mut(fi))
            {
                face.hidden = true;
                rebuild.insert((li, oi));
            }
        }
        for (li, oi) in rebuild {
            scene.layers[li].objects[oi].rebuild_gpu_mesh(device);
        }
    }

    fn undo(&mut self, scene: &mut Scene, device: &wgpu::Device) {
        let mut rebuild: std::collections::HashSet<(usize, usize)> = std::collections::HashSet::new();
        for &(li, oi, fi) in &self.faces {
            if let Some(face) = scene.layers.get_mut(li)
                .and_then(|l| l.objects.get_mut(oi))
                .and_then(|o| o.faces.get_mut(fi))
            {
                face.hidden = false;
                rebuild.insert((li, oi));
            }
        }
        for (li, oi) in rebuild {
            scene.layers[li].objects[oi].rebuild_gpu_mesh(device);
        }
    }

    fn description(&self) -> &str {
        "Hide Faces"
    }
}

/// Show all hidden faces (undoable). Stores which faces were hidden for undo.
pub struct ShowAllFaces {
    pub previously_hidden: Vec<(usize, usize, usize)>,
}

impl Command for ShowAllFaces {
    fn apply(&mut self, scene: &mut Scene, device: &wgpu::Device) {
        let mut rebuild: std::collections::HashSet<(usize, usize)> = std::collections::HashSet::new();
        for &(li, oi, fi) in &self.previously_hidden {
            if let Some(face) = scene.layers.get_mut(li)
                .and_then(|l| l.objects.get_mut(oi))
                .and_then(|o| o.faces.get_mut(fi))
            {
                face.hidden = false;
                rebuild.insert((li, oi));
            }
        }
        for (li, oi) in rebuild {
            scene.layers[li].objects[oi].rebuild_gpu_mesh(device);
        }
    }

    fn undo(&mut self, scene: &mut Scene, device: &wgpu::Device) {
        let mut rebuild: std::collections::HashSet<(usize, usize)> = std::collections::HashSet::new();
        for &(li, oi, fi) in &self.previously_hidden {
            if let Some(face) = scene.layers.get_mut(li)
                .and_then(|l| l.objects.get_mut(oi))
                .and_then(|o| o.faces.get_mut(fi))
            {
                face.hidden = true;
                rebuild.insert((li, oi));
            }
        }
        for (li, oi) in rebuild {
            scene.layers[li].objects[oi].rebuild_gpu_mesh(device);
        }
    }

    fn description(&self) -> &str {
        "Show All Faces"
    }
}

/// Edit a face's properties (positions, UVs, colors) with undo support.
pub struct EditFaceProperty {
    pub face: (usize, usize, usize),
    pub old_positions: [Vec3; 4],
    pub old_uvs: [Vec2; 4],
    pub old_colors: [Vec4; 4],
    pub new_positions: [Vec3; 4],
    pub new_uvs: [Vec2; 4],
    pub new_colors: [Vec4; 4],
}

impl Command for EditFaceProperty {
    fn apply(&mut self, scene: &mut Scene, device: &wgpu::Device) {
        let (li, oi, fi) = self.face;
        if let Some(face) = scene.layers.get_mut(li)
            .and_then(|l| l.objects.get_mut(oi))
            .and_then(|o| o.faces.get_mut(fi))
        {
            face.positions = self.new_positions;
            face.uvs = self.new_uvs;
            face.colors = self.new_colors;
        }
        scene.layers[li].objects[oi].rebuild_gpu_mesh(device);
    }

    fn undo(&mut self, scene: &mut Scene, device: &wgpu::Device) {
        let (li, oi, fi) = self.face;
        if let Some(face) = scene.layers.get_mut(li)
            .and_then(|l| l.objects.get_mut(oi))
            .and_then(|o| o.faces.get_mut(fi))
        {
            face.positions = self.old_positions;
            face.uvs = self.old_uvs;
            face.colors = self.old_colors;
        }
        scene.layers[li].objects[oi].rebuild_gpu_mesh(device);
    }

    fn description(&self) -> &str {
        "Edit Face Property"
    }
}

/// Manipulate UVs of selected faces (rotate, flip).
pub struct ManipulateUVs {
    pub faces: Vec<(usize, usize, usize)>,
    pub old_uvs: Vec<[Vec2; 4]>,
    pub new_uvs: Vec<[Vec2; 4]>,
}

impl Command for ManipulateUVs {
    fn apply(&mut self, scene: &mut Scene, device: &wgpu::Device) {
        let mut rebuild: std::collections::HashSet<(usize, usize)> = std::collections::HashSet::new();
        for (i, &(li, oi, fi)) in self.faces.iter().enumerate() {
            if let Some(face) = scene.layers.get_mut(li)
                .and_then(|l| l.objects.get_mut(oi))
                .and_then(|o| o.faces.get_mut(fi))
            {
                face.uvs = self.new_uvs[i];
                rebuild.insert((li, oi));
            }
        }
        for (li, oi) in rebuild {
            scene.layers[li].objects[oi].rebuild_gpu_mesh(device);
        }
    }

    fn undo(&mut self, scene: &mut Scene, device: &wgpu::Device) {
        let mut rebuild: std::collections::HashSet<(usize, usize)> = std::collections::HashSet::new();
        for (i, &(li, oi, fi)) in self.faces.iter().enumerate() {
            if let Some(face) = scene.layers.get_mut(li)
                .and_then(|l| l.objects.get_mut(oi))
                .and_then(|o| o.faces.get_mut(fi))
            {
                face.uvs = self.old_uvs[i];
                rebuild.insert((li, oi));
            }
        }
        for (li, oi) in rebuild {
            scene.layers[li].objects[oi].rebuild_gpu_mesh(device);
        }
    }

    fn description(&self) -> &str {
        "Manipulate UVs"
    }
}

/// Merge vertices by moving them to new positions.
pub struct MergeVertices {
    pub moves: Vec<(usize, usize, usize, usize, Vec3, Vec3)>, // (li, oi, fi, vi, old_pos, new_pos)
}

impl Command for MergeVertices {
    fn apply(&mut self, scene: &mut Scene, device: &wgpu::Device) {
        let mut rebuild: std::collections::HashSet<(usize, usize)> = std::collections::HashSet::new();
        for &(li, oi, fi, vi, _, new_pos) in &self.moves {
            if let Some(face) = scene.layers.get_mut(li)
                .and_then(|l| l.objects.get_mut(oi))
                .and_then(|o| o.faces.get_mut(fi))
            {
                face.positions[vi] = new_pos;
                rebuild.insert((li, oi));
            }
        }
        for (li, oi) in rebuild {
            scene.layers[li].objects[oi].rebuild_gpu_mesh(device);
        }
    }

    fn undo(&mut self, scene: &mut Scene, device: &wgpu::Device) {
        let mut rebuild: std::collections::HashSet<(usize, usize)> = std::collections::HashSet::new();
        for &(li, oi, fi, vi, old_pos, _) in &self.moves {
            if let Some(face) = scene.layers.get_mut(li)
                .and_then(|l| l.objects.get_mut(oi))
                .and_then(|o| o.faces.get_mut(fi))
            {
                face.positions[vi] = old_pos;
                rebuild.insert((li, oi));
            }
        }
        for (li, oi) in rebuild {
            scene.layers[li].objects[oi].rebuild_gpu_mesh(device);
        }
    }

    fn description(&self) -> &str {
        "Merge Vertices"
    }
}

/// Split an edge, turning one quad into two quads.
pub struct SplitEdge {
    pub targets: Vec<(usize, usize, usize, usize)>, // (li, oi, fi, edge_idx)
    original_faces: Vec<(usize, usize, usize, Face)>,
    added_per_object: Vec<(usize, usize, usize)>, // (li, oi, count)
}

impl SplitEdge {
    pub fn new(targets: Vec<(usize, usize, usize, usize)>) -> Self {
        Self { targets, original_faces: Vec::new(), added_per_object: Vec::new() }
    }
}

impl Command for SplitEdge {
    fn apply(&mut self, scene: &mut Scene, device: &wgpu::Device) {
        self.original_faces.clear();
        self.added_per_object.clear();
        let mut adds_per_obj: std::collections::HashMap<(usize, usize), usize> = std::collections::HashMap::new();

        // Process in reverse face order to avoid index shifting
        let mut sorted = self.targets.clone();
        sorted.sort_by(|a, b| b.2.cmp(&a.2));

        for &(li, oi, fi, edge_idx) in &sorted {
            let face = scene.layers[li].objects[oi].faces[fi].clone();
            self.original_faces.push((li, oi, fi, face.clone()));

            let p = face.positions;
            let uv = face.uvs;
            let c = face.colors;
            let e = edge_idx;
            let en = (e + 1) % 4;
            let opp = (e + 2) % 4;
            let oppn = (e + 3) % 4;

            // Midpoint on the selected edge
            let mid_p = (p[e] + p[en]) * 0.5;
            let mid_uv = (uv[e] + uv[en]) * 0.5;
            let mid_c = (c[e] + c[en]) * 0.5;

            // Midpoint on the opposite edge
            let mid_opp_p = (p[opp] + p[oppn]) * 0.5;
            let mid_opp_uv = (uv[opp] + uv[oppn]) * 0.5;
            let mid_opp_c = (c[opp] + c[oppn]) * 0.5;

            // Two new quads
            let face_a = Face {
                positions: [p[e], mid_p, mid_opp_p, p[oppn]],
                uvs: [uv[e], mid_uv, mid_opp_uv, uv[oppn]],
                colors: [c[e], mid_c, mid_opp_c, c[oppn]],
                hidden: false,
            };
            let face_b = Face {
                positions: [mid_p, p[en], p[opp], mid_opp_p],
                uvs: [mid_uv, uv[en], uv[opp], mid_opp_uv],
                colors: [mid_c, c[en], c[opp], mid_opp_c],
                hidden: false,
            };

            scene.layers[li].objects[oi].faces.remove(fi);
            scene.layers[li].objects[oi].faces.push(face_a);
            scene.layers[li].objects[oi].faces.push(face_b);
            *adds_per_obj.entry((li, oi)).or_insert(0) += 2;
        }

        for ((li, oi), count) in &adds_per_obj {
            self.added_per_object.push((*li, *oi, *count));
        }

        let mut rebuild: std::collections::HashSet<(usize, usize)> = std::collections::HashSet::new();
        for &(li, oi, _, _) in &self.targets {
            rebuild.insert((li, oi));
        }
        for (li, oi) in rebuild {
            scene.layers[li].objects[oi].rebuild_gpu_mesh(device);
        }
    }

    fn undo(&mut self, scene: &mut Scene, device: &wgpu::Device) {
        // Remove added faces
        for &(li, oi, count) in &self.added_per_object {
            for _ in 0..count {
                scene.layers[li].objects[oi].faces.pop();
            }
        }

        // Re-insert original faces (stored in reverse order, so insert in reverse)
        for (li, oi, fi, face) in self.original_faces.iter().rev() {
            scene.layers[*li].objects[*oi].faces.insert(*fi, face.clone());
        }

        let mut rebuild: std::collections::HashSet<(usize, usize)> = std::collections::HashSet::new();
        for &(li, oi, _, _) in &self.targets {
            rebuild.insert((li, oi));
        }
        for (li, oi) in rebuild {
            scene.layers[li].objects[oi].rebuild_gpu_mesh(device);
        }
    }

    fn description(&self) -> &str {
        "Split Edge"
    }
}

/// Collapse an edge by merging its two vertices to their midpoint.
pub struct CollapseEdge {
    pub targets: Vec<(usize, usize, usize, usize)>, // (li, oi, fi, edge_idx)
    old_positions: Vec<(usize, usize, usize, [Vec3; 4])>,
}

impl CollapseEdge {
    pub fn new(targets: Vec<(usize, usize, usize, usize)>) -> Self {
        Self { targets, old_positions: Vec::new() }
    }
}

impl Command for CollapseEdge {
    fn apply(&mut self, scene: &mut Scene, device: &wgpu::Device) {
        self.old_positions.clear();
        let mut rebuild: std::collections::HashSet<(usize, usize)> = std::collections::HashSet::new();

        for &(li, oi, fi, edge_idx) in &self.targets {
            let face = &scene.layers[li].objects[oi].faces[fi];
            self.old_positions.push((li, oi, fi, face.positions));

            let en = (edge_idx + 1) % 4;
            let mid = (face.positions[edge_idx] + face.positions[en]) * 0.5;
            let face = &mut scene.layers[li].objects[oi].faces[fi];
            face.positions[edge_idx] = mid;
            face.positions[en] = mid;
            rebuild.insert((li, oi));
        }

        for (li, oi) in rebuild {
            scene.layers[li].objects[oi].rebuild_gpu_mesh(device);
        }
    }

    fn undo(&mut self, scene: &mut Scene, device: &wgpu::Device) {
        let mut rebuild: std::collections::HashSet<(usize, usize)> = std::collections::HashSet::new();
        for &(li, oi, fi, positions) in &self.old_positions {
            scene.layers[li].objects[oi].faces[fi].positions = positions;
            rebuild.insert((li, oi));
        }
        for (li, oi) in rebuild {
            scene.layers[li].objects[oi].rebuild_gpu_mesh(device);
        }
    }

    fn description(&self) -> &str {
        "Collapse Edge"
    }
}

/// Place one or more tile faces into a specific layer/object.
pub struct PlaceTile {
    pub layer: usize,
    pub object: usize,
    pub faces: Vec<Face>,
    /// If true, we need to create the object on apply (first time only).
    pub create_object: bool,
    /// Active tileset index at time of placement.
    pub tileset_index: Option<usize>,
}

impl Command for PlaceTile {
    fn apply(&mut self, scene: &mut Scene, device: &wgpu::Device) {
        let layer = &mut scene.layers[self.layer];
        if self.create_object && layer.objects.len() <= self.object {
            layer.objects.push(Object::new(format!("Object {}", self.object + 1)));
            self.create_object = false;
        }
        let object = &mut layer.objects[self.object];
        for face in &self.faces {
            object.faces.push(face.clone());
        }
        if let Some(ts_idx) = self.tileset_index {
            object.tileset_index = Some(ts_idx);
        }
        object.rebuild_gpu_mesh(device);
    }

    fn undo(&mut self, scene: &mut Scene, device: &wgpu::Device) {
        let object = &mut scene.layers[self.layer].objects[self.object];
        for _ in 0..self.faces.len() {
            object.faces.pop();
        }
        object.rebuild_gpu_mesh(device);
    }

    fn description(&self) -> &str {
        "Place Tile"
    }
}

/// Erase a tile face from a specific layer/object at a specific index.
pub struct EraseTile {
    pub layer: usize,
    pub object: usize,
    pub face_index: usize,
    pub face: Face,
}

impl Command for EraseTile {
    fn apply(&mut self, scene: &mut Scene, device: &wgpu::Device) {
        let object = &mut scene.layers[self.layer].objects[self.object];
        object.faces.remove(self.face_index);
        object.rebuild_gpu_mesh(device);
    }

    fn undo(&mut self, scene: &mut Scene, device: &wgpu::Device) {
        let object = &mut scene.layers[self.layer].objects[self.object];
        object.faces.insert(self.face_index, self.face.clone());
        object.rebuild_gpu_mesh(device);
    }

    fn description(&self) -> &str {
        "Erase Tile"
    }
}

/// Translate selected faces/objects/vertices by a delta.
pub struct TranslateSelection {
    pub faces: Vec<(usize, usize, usize)>,
    pub objects: Vec<(usize, usize)>,
    pub vertices: Vec<(usize, usize, usize, usize)>,
    pub delta: Vec3,
}

impl Command for TranslateSelection {
    fn apply(&mut self, scene: &mut Scene, device: &wgpu::Device) {
        self.translate(scene, device, self.delta);
    }

    fn undo(&mut self, scene: &mut Scene, device: &wgpu::Device) {
        self.translate(scene, device, -self.delta);
    }

    fn description(&self) -> &str {
        "Translate Selection"
    }
}

impl TranslateSelection {
    fn translate(&self, scene: &mut Scene, device: &wgpu::Device, delta: Vec3) {
        let mut rebuild: std::collections::HashSet<(usize, usize)> = std::collections::HashSet::new();

        for &(li, oi, fi) in &self.faces {
            let face = &mut scene.layers[li].objects[oi].faces[fi];
            for pos in &mut face.positions {
                *pos += delta;
            }
            rebuild.insert((li, oi));
        }

        for &(li, oi) in &self.objects {
            for face in &mut scene.layers[li].objects[oi].faces {
                for pos in &mut face.positions {
                    *pos += delta;
                }
            }
            rebuild.insert((li, oi));
        }

        for &(li, oi, fi, vi) in &self.vertices {
            scene.layers[li].objects[oi].faces[fi].positions[vi] += delta;
            rebuild.insert((li, oi));
        }

        for (li, oi) in rebuild {
            scene.layers[li].objects[oi].rebuild_gpu_mesh(device);
        }
    }
}

/// Rotate selected faces/objects around a center point by an angle about an axis.
pub struct RotateSelection {
    pub faces: Vec<(usize, usize, usize)>,
    pub objects: Vec<(usize, usize)>,
    pub vertices: Vec<(usize, usize, usize, usize)>,
    pub axis: Vec3,
    pub angle: f32,
    pub center: Vec3,
}

impl Command for RotateSelection {
    fn apply(&mut self, scene: &mut Scene, device: &wgpu::Device) {
        self.rotate(scene, device, self.angle);
    }

    fn undo(&mut self, scene: &mut Scene, device: &wgpu::Device) {
        self.rotate(scene, device, -self.angle);
    }

    fn description(&self) -> &str {
        "Rotate Selection"
    }
}

impl RotateSelection {
    fn rotate(&self, scene: &mut Scene, device: &wgpu::Device, angle: f32) {
        let quat = Quat::from_axis_angle(self.axis, angle);
        let mut rebuild: std::collections::HashSet<(usize, usize)> = std::collections::HashSet::new();

        for &(li, oi, fi) in &self.faces {
            let face = &mut scene.layers[li].objects[oi].faces[fi];
            for pos in &mut face.positions {
                *pos = quat * (*pos - self.center) + self.center;
            }
            rebuild.insert((li, oi));
        }

        for &(li, oi) in &self.objects {
            for face in &mut scene.layers[li].objects[oi].faces {
                for pos in &mut face.positions {
                    *pos = quat * (*pos - self.center) + self.center;
                }
            }
            rebuild.insert((li, oi));
        }

        for &(li, oi, fi, vi) in &self.vertices {
            let pos = &mut scene.layers[li].objects[oi].faces[fi].positions[vi];
            *pos = quat * (*pos - self.center) + self.center;
            rebuild.insert((li, oi));
        }

        for (li, oi) in rebuild {
            scene.layers[li].objects[oi].rebuild_gpu_mesh(device);
        }
    }
}

/// Flip normals (reverse winding order) of selected faces.
pub struct FlipNormals {
    pub faces: Vec<(usize, usize, usize)>,
    pub objects: Vec<(usize, usize)>,
}

impl Command for FlipNormals {
    fn apply(&mut self, scene: &mut Scene, device: &wgpu::Device) {
        self.flip(scene, device);
    }

    fn undo(&mut self, scene: &mut Scene, device: &wgpu::Device) {
        self.flip(scene, device); // Self-inverse
    }

    fn description(&self) -> &str {
        "Flip Normals"
    }
}

impl FlipNormals {
    fn flip(&self, scene: &mut Scene, device: &wgpu::Device) {
        let mut rebuild: std::collections::HashSet<(usize, usize)> = std::collections::HashSet::new();

        for &(li, oi, fi) in &self.faces {
            let face = &mut scene.layers[li].objects[oi].faces[fi];
            face.positions.swap(1, 3);
            face.uvs.swap(1, 3);
            face.colors.swap(1, 3);
            rebuild.insert((li, oi));
        }

        for &(li, oi) in &self.objects {
            for face in &mut scene.layers[li].objects[oi].faces {
                face.positions.swap(1, 3);
                face.uvs.swap(1, 3);
                face.colors.swap(1, 3);
            }
            rebuild.insert((li, oi));
        }

        for (li, oi) in rebuild {
            scene.layers[li].objects[oi].rebuild_gpu_mesh(device);
        }
    }
}

/// Extrude selected faces along their normals, creating side walls.
pub struct ExtrudeFaces {
    pub face_indices: Vec<(usize, usize, usize)>,
    pub distance: f32,
    /// Populated during apply: original positions for each face (for undo).
    original_positions: Vec<[Vec3; 4]>,
    /// Number of side faces added per (layer, object).
    sides_added: Vec<(usize, usize, usize)>,
}

impl ExtrudeFaces {
    pub fn new(face_indices: Vec<(usize, usize, usize)>, distance: f32) -> Self {
        Self {
            face_indices,
            distance,
            original_positions: Vec::new(),
            sides_added: Vec::new(),
        }
    }
}

impl Command for ExtrudeFaces {
    fn apply(&mut self, scene: &mut Scene, device: &wgpu::Device) {
        self.original_positions.clear();
        self.sides_added.clear();

        // Count side faces per object
        let mut sides_per_obj: std::collections::HashMap<(usize, usize), usize> =
            std::collections::HashMap::new();

        for &(li, oi, fi) in &self.face_indices {
            let face = &scene.layers[li].objects[oi].faces[fi];
            let normal = face.normal();
            let orig = face.positions;
            self.original_positions.push(orig);

            // Move face outward
            let offset = normal * self.distance;
            let face_mut = &mut scene.layers[li].objects[oi].faces[fi];
            for pos in &mut face_mut.positions {
                *pos += offset;
            }
            let new_positions = scene.layers[li].objects[oi].faces[fi].positions;

            // Create 4 side faces connecting original edge positions to new positions
            for edge in 0..4 {
                let next = (edge + 1) % 4;
                let side = Face {
                    positions: [orig[edge], orig[next], new_positions[next], new_positions[edge]],
                    uvs: default_uvs(),
                    colors: [Vec4::ONE; 4],
                    hidden: false,
                };
                scene.layers[li].objects[oi].faces.push(side);
            }
            *sides_per_obj.entry((li, oi)).or_insert(0) += 4;
        }

        // Store sides added info
        for ((li, oi), count) in &sides_per_obj {
            self.sides_added.push((*li, *oi, *count));
        }

        // Rebuild affected objects
        let mut rebuild: std::collections::HashSet<(usize, usize)> = std::collections::HashSet::new();
        for &(li, oi, _) in &self.face_indices {
            rebuild.insert((li, oi));
        }
        for (li, oi) in rebuild {
            scene.layers[li].objects[oi].rebuild_gpu_mesh(device);
        }
    }

    fn undo(&mut self, scene: &mut Scene, device: &wgpu::Device) {
        // Remove side faces (pop from end of each object)
        for &(li, oi, count) in &self.sides_added {
            for _ in 0..count {
                scene.layers[li].objects[oi].faces.pop();
            }
        }

        // Restore original positions
        for (i, &(li, oi, fi)) in self.face_indices.iter().enumerate() {
            if let Some(orig) = self.original_positions.get(i) {
                scene.layers[li].objects[oi].faces[fi].positions = *orig;
            }
        }

        // Rebuild
        let mut rebuild: std::collections::HashSet<(usize, usize)> = std::collections::HashSet::new();
        for &(li, oi, _) in &self.face_indices {
            rebuild.insert((li, oi));
        }
        for (li, oi) in rebuild {
            scene.layers[li].objects[oi].rebuild_gpu_mesh(device);
        }
    }

    fn description(&self) -> &str {
        "Extrude Faces"
    }
}

/// Delete selected faces/objects, storing them for undo.
pub struct DeleteSelection {
    pub removed_faces: Vec<(usize, usize, usize, Face)>,
    pub removed_objects: Vec<(usize, usize, String, Vec<Face>)>,
}

impl Command for DeleteSelection {
    fn apply(&mut self, scene: &mut Scene, device: &wgpu::Device) {
        let mut rebuild: std::collections::HashSet<(usize, usize)> = std::collections::HashSet::new();

        // Remove faces (sorted in reverse order to preserve indices)
        let mut faces = self.removed_faces.iter().map(|(l, o, f, _)| (*l, *o, *f)).collect::<Vec<_>>();
        faces.sort_by(|a, b| b.2.cmp(&a.2));
        for (li, oi, fi) in faces {
            scene.layers[li].objects[oi].faces.remove(fi);
            rebuild.insert((li, oi));
        }

        // Remove objects (sorted in reverse order)
        let mut objs = self.removed_objects.iter().map(|(l, o, _, _)| (*l, *o)).collect::<Vec<_>>();
        objs.sort_by(|a, b| b.1.cmp(&a.1));
        for (li, oi) in objs {
            scene.layers[li].objects.remove(oi);
        }

        for (li, oi) in rebuild {
            if oi < scene.layers[li].objects.len() {
                scene.layers[li].objects[oi].rebuild_gpu_mesh(device);
            }
        }
    }

    fn undo(&mut self, scene: &mut Scene, device: &wgpu::Device) {
        // Re-insert objects (in forward order)
        for (li, oi, name, faces) in &self.removed_objects {
            let mut obj = Object::new(name.clone());
            obj.faces = faces.clone();
            scene.layers[*li].objects.insert(*oi, obj);
        }

        // Re-insert faces (in forward order)
        let mut faces_sorted = self.removed_faces.clone();
        faces_sorted.sort_by_key(|(_, _, fi, _)| *fi);
        for (li, oi, fi, face) in &faces_sorted {
            scene.layers[*li].objects[*oi].faces.insert(*fi, face.clone());
        }

        // Rebuild all affected objects
        let mut rebuild: std::collections::HashSet<(usize, usize)> = std::collections::HashSet::new();
        for (li, oi, _, _) in &self.removed_faces {
            rebuild.insert((*li, *oi));
        }
        for (li, oi, _, _) in &self.removed_objects {
            rebuild.insert((*li, *oi));
        }
        for (li, oi) in rebuild {
            if oi < scene.layers[li].objects.len() {
                scene.layers[li].objects[oi].rebuild_gpu_mesh(device);
            }
        }
    }

    fn description(&self) -> &str {
        "Delete Selection"
    }
}

/// Scale selected faces/objects/vertices around a center point.
pub struct ScaleSelection {
    pub faces: Vec<(usize, usize, usize)>,
    pub objects: Vec<(usize, usize)>,
    pub vertices: Vec<(usize, usize, usize, usize)>,
    pub scale_factor: Vec3,
    pub center: Vec3,
}

impl Command for ScaleSelection {
    fn apply(&mut self, scene: &mut Scene, device: &wgpu::Device) {
        self.scale(scene, device, self.scale_factor);
    }

    fn undo(&mut self, scene: &mut Scene, device: &wgpu::Device) {
        let inv = Vec3::new(1.0 / self.scale_factor.x, 1.0 / self.scale_factor.y, 1.0 / self.scale_factor.z);
        self.scale(scene, device, inv);
    }

    fn description(&self) -> &str {
        "Scale Selection"
    }
}

impl ScaleSelection {
    fn scale(&self, scene: &mut Scene, device: &wgpu::Device, factor: Vec3) {
        let mut rebuild: std::collections::HashSet<(usize, usize)> = std::collections::HashSet::new();

        for &(li, oi, fi) in &self.faces {
            let face = &mut scene.layers[li].objects[oi].faces[fi];
            for pos in &mut face.positions {
                *pos = self.center + (*pos - self.center) * factor;
            }
            rebuild.insert((li, oi));
        }

        for &(li, oi) in &self.objects {
            for face in &mut scene.layers[li].objects[oi].faces {
                for pos in &mut face.positions {
                    *pos = self.center + (*pos - self.center) * factor;
                }
            }
            rebuild.insert((li, oi));
        }

        for &(li, oi, fi, vi) in &self.vertices {
            let pos = &mut scene.layers[li].objects[oi].faces[fi].positions[vi];
            *pos = self.center + (*pos - self.center) * factor;
            rebuild.insert((li, oi));
        }

        for (li, oi) in rebuild {
            scene.layers[li].objects[oi].rebuild_gpu_mesh(device);
        }
    }
}

/// Retile: apply new UVs to selected faces.
pub struct RetileFaces {
    pub faces: Vec<(usize, usize, usize)>,
    pub new_uvs: [Vec2; 4],
    pub old_uvs: Vec<[Vec2; 4]>,
}

impl Command for RetileFaces {
    fn apply(&mut self, scene: &mut Scene, device: &wgpu::Device) {
        let mut rebuild: std::collections::HashSet<(usize, usize)> = std::collections::HashSet::new();
        self.old_uvs.clear();

        for &(li, oi, fi) in &self.faces {
            let face = &mut scene.layers[li].objects[oi].faces[fi];
            self.old_uvs.push(face.uvs);
            face.uvs = self.new_uvs;
            rebuild.insert((li, oi));
        }

        for (li, oi) in rebuild {
            scene.layers[li].objects[oi].rebuild_gpu_mesh(device);
        }
    }

    fn undo(&mut self, scene: &mut Scene, device: &wgpu::Device) {
        let mut rebuild: std::collections::HashSet<(usize, usize)> = std::collections::HashSet::new();

        for (i, &(li, oi, fi)) in self.faces.iter().enumerate() {
            if let Some(old) = self.old_uvs.get(i) {
                scene.layers[li].objects[oi].faces[fi].uvs = *old;
                rebuild.insert((li, oi));
            }
        }

        for (li, oi) in rebuild {
            scene.layers[li].objects[oi].rebuild_gpu_mesh(device);
        }
    }

    fn description(&self) -> &str {
        "Retile Faces"
    }
}

/// Paint vertex colors on selected faces.
pub struct PaintVertexColor {
    pub targets: Vec<(usize, usize, usize)>,
    pub new_color: Vec4,
    pub old_colors: Vec<[Vec4; 4]>,
}

impl Command for PaintVertexColor {
    fn apply(&mut self, scene: &mut Scene, device: &wgpu::Device) {
        let mut rebuild: std::collections::HashSet<(usize, usize)> = std::collections::HashSet::new();
        self.old_colors.clear();

        for &(li, oi, fi) in &self.targets {
            let face = &mut scene.layers[li].objects[oi].faces[fi];
            self.old_colors.push(face.colors);
            face.colors = [self.new_color; 4];
            rebuild.insert((li, oi));
        }

        for (li, oi) in rebuild {
            scene.layers[li].objects[oi].rebuild_gpu_mesh(device);
        }
    }

    fn undo(&mut self, scene: &mut Scene, device: &wgpu::Device) {
        let mut rebuild: std::collections::HashSet<(usize, usize)> = std::collections::HashSet::new();

        for (i, &(li, oi, fi)) in self.targets.iter().enumerate() {
            if let Some(old) = self.old_colors.get(i) {
                scene.layers[li].objects[oi].faces[fi].colors = *old;
                rebuild.insert((li, oi));
            }
        }

        for (li, oi) in rebuild {
            scene.layers[li].objects[oi].rebuild_gpu_mesh(device);
        }
    }

    fn description(&self) -> &str {
        "Paint Vertex Color"
    }
}

/// Subdivide selected faces into 4 sub-quads each.
pub struct SubdivideFaces {
    pub faces: Vec<(usize, usize, usize)>,
    /// For undo: store the original face and the number of new faces added per object.
    original_faces: Vec<Face>,
    added_per_object: Vec<(usize, usize, usize)>, // (li, oi, count_added)
}

impl SubdivideFaces {
    pub fn new(faces: Vec<(usize, usize, usize)>) -> Self {
        Self { faces, original_faces: Vec::new(), added_per_object: Vec::new() }
    }
}

impl Command for SubdivideFaces {
    fn apply(&mut self, scene: &mut Scene, device: &wgpu::Device) {
        self.original_faces.clear();
        self.added_per_object.clear();
        let mut adds_per_obj: std::collections::HashMap<(usize, usize), usize> = std::collections::HashMap::new();

        // Process faces in reverse index order so removals don't shift earlier indices
        let mut sorted = self.faces.clone();
        sorted.sort_by(|a, b| b.2.cmp(&a.2));

        for &(li, oi, fi) in &sorted {
            let face = scene.layers[li].objects[oi].faces[fi].clone();
            self.original_faces.push(face.clone());

            let p = face.positions;
            let uv = face.uvs;
            let c = face.colors;

            // Midpoints
            let m01 = (p[0] + p[1]) * 0.5;
            let m12 = (p[1] + p[2]) * 0.5;
            let m23 = (p[2] + p[3]) * 0.5;
            let m30 = (p[3] + p[0]) * 0.5;
            let center = (p[0] + p[1] + p[2] + p[3]) * 0.25;

            let uvm01 = (uv[0] + uv[1]) * 0.5;
            let uvm12 = (uv[1] + uv[2]) * 0.5;
            let uvm23 = (uv[2] + uv[3]) * 0.5;
            let uvm30 = (uv[3] + uv[0]) * 0.5;
            let uvc = (uv[0] + uv[1] + uv[2] + uv[3]) * 0.25;

            let cm01 = (c[0] + c[1]) * 0.5;
            let cm12 = (c[1] + c[2]) * 0.5;
            let cm23 = (c[2] + c[3]) * 0.5;
            let cm30 = (c[3] + c[0]) * 0.5;
            let cc = (c[0] + c[1] + c[2] + c[3]) * 0.25;

            let sub_faces = [
                Face { positions: [p[0], m01, center, m30], uvs: [uv[0], uvm01, uvc, uvm30], colors: [c[0], cm01, cc, cm30], hidden: false },
                Face { positions: [m01, p[1], m12, center], uvs: [uvm01, uv[1], uvm12, uvc], colors: [cm01, c[1], cm12, cc], hidden: false },
                Face { positions: [center, m12, p[2], m23], uvs: [uvc, uvm12, uv[2], uvm23], colors: [cc, cm12, c[2], cm23], hidden: false },
                Face { positions: [m30, center, m23, p[3]], uvs: [uvm30, uvc, uvm23, uv[3]], colors: [cm30, cc, cm23, c[3]], hidden: false },
            ];

            // Remove original face, add 4 new ones
            scene.layers[li].objects[oi].faces.remove(fi);
            for sf in sub_faces {
                scene.layers[li].objects[oi].faces.push(sf);
            }
            *adds_per_obj.entry((li, oi)).or_insert(0) += 4;
        }

        for ((li, oi), count) in &adds_per_obj {
            self.added_per_object.push((*li, *oi, *count));
        }

        let mut rebuild: std::collections::HashSet<(usize, usize)> = std::collections::HashSet::new();
        for &(li, oi, _) in &self.faces {
            rebuild.insert((li, oi));
        }
        for (li, oi) in rebuild {
            scene.layers[li].objects[oi].rebuild_gpu_mesh(device);
        }
    }

    fn undo(&mut self, scene: &mut Scene, device: &wgpu::Device) {
        // Remove added faces (pop from end)
        for &(li, oi, count) in &self.added_per_object {
            for _ in 0..count {
                scene.layers[li].objects[oi].faces.pop();
            }
        }

        // Re-insert original faces in forward order (they were stored in reverse)
        let mut sorted = self.faces.clone();
        sorted.sort_by(|a, b| b.2.cmp(&a.2));
        for (i, &(li, oi, fi)) in sorted.iter().enumerate() {
            if let Some(orig) = self.original_faces.get(i) {
                scene.layers[li].objects[oi].faces.insert(fi, orig.clone());
            }
        }

        let mut rebuild: std::collections::HashSet<(usize, usize)> = std::collections::HashSet::new();
        for &(li, oi, _) in &self.faces {
            rebuild.insert((li, oi));
        }
        for (li, oi) in rebuild {
            scene.layers[li].objects[oi].rebuild_gpu_mesh(device);
        }
    }

    fn description(&self) -> &str {
        "Subdivide Faces"
    }
}

/// Create a new object from selected faces, moving them out of their current objects.
pub struct CreateObjectFromSelection {
    pub faces: Vec<(usize, usize, usize)>,
    pub target_layer: usize,
    pub object_name: String,
    /// Stored during apply for undo
    moved_faces: Vec<(usize, usize, usize, Face)>,
    created_object_index: Option<usize>,
}

impl CreateObjectFromSelection {
    pub fn new(faces: Vec<(usize, usize, usize)>, target_layer: usize, name: String) -> Self {
        Self { faces, target_layer, object_name: name, moved_faces: Vec::new(), created_object_index: None }
    }
}

impl Command for CreateObjectFromSelection {
    fn apply(&mut self, scene: &mut Scene, device: &wgpu::Device) {
        self.moved_faces.clear();

        // Collect faces to move (sorted in reverse to preserve indices during removal)
        let mut sorted = self.faces.clone();
        sorted.sort_by(|a, b| b.cmp(a));

        let mut collected_faces = Vec::new();
        for &(li, oi, fi) in &sorted {
            let face = scene.layers[li].objects[oi].faces.remove(fi);
            self.moved_faces.push((li, oi, fi, face.clone()));
            collected_faces.push(face);
        }
        collected_faces.reverse(); // Back to original order

        // Create new object
        let mut new_obj = Object::new(self.object_name.clone());
        new_obj.faces = collected_faces;
        new_obj.rebuild_gpu_mesh(device);
        scene.layers[self.target_layer].objects.push(new_obj);
        self.created_object_index = Some(scene.layers[self.target_layer].objects.len() - 1);

        // Rebuild source objects
        let mut rebuild: std::collections::HashSet<(usize, usize)> = std::collections::HashSet::new();
        for &(li, oi, _, _) in &self.moved_faces {
            rebuild.insert((li, oi));
        }
        for (li, oi) in rebuild {
            scene.layers[li].objects[oi].rebuild_gpu_mesh(device);
        }
    }

    fn undo(&mut self, scene: &mut Scene, device: &wgpu::Device) {
        // Remove created object
        if let Some(idx) = self.created_object_index {
            scene.layers[self.target_layer].objects.remove(idx);
        }

        // Re-insert faces back into their original objects (forward order, ascending fi)
        let mut to_restore = self.moved_faces.clone();
        to_restore.sort_by_key(|(_, _, fi, _)| *fi);
        for (li, oi, fi, face) in to_restore {
            scene.layers[li].objects[oi].faces.insert(fi, face);
        }

        // Rebuild all affected objects
        let mut rebuild: std::collections::HashSet<(usize, usize)> = std::collections::HashSet::new();
        for &(li, oi, _, _) in &self.moved_faces {
            rebuild.insert((li, oi));
        }
        for (li, oi) in rebuild {
            scene.layers[li].objects[oi].rebuild_gpu_mesh(device);
        }
    }

    fn description(&self) -> &str {
        "Create Object"
    }
}
