use glam::{Mat4, Vec2, Vec3};
use crate::render::gizmo::{GizmoAxis, GizmoDrag};
use crate::scene::Scene;
use crate::util::picking::{self, project_to_screen, Ray};

/// Selection level for edit mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionLevel {
    Object,
    Face,
    Vertex,
    Edge,
}

/// Which transform gizmo is active.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GizmoMode {
    Translate,
    Rotate,
    Scale,
}

/// Tracks what is currently selected in Edit mode.
#[derive(Debug, Clone, Default)]
pub struct Selection {
    /// (layer_index, object_index) pairs
    pub objects: Vec<(usize, usize)>,
    /// (layer_index, object_index, face_index) triples
    pub faces: Vec<(usize, usize, usize)>,
    /// (layer_index, object_index, face_index, vertex_index_within_face) quads
    pub vertices: Vec<(usize, usize, usize, usize)>,
    /// (layer_index, object_index, face_index, edge_index) for edge selection
    pub edges: Vec<(usize, usize, usize, usize)>,
}

impl Selection {
    pub fn clear(&mut self) {
        self.objects.clear();
        self.faces.clear();
        self.vertices.clear();
        self.edges.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.objects.is_empty() && self.faces.is_empty() && self.vertices.is_empty() && self.edges.is_empty()
    }

    /// Compute the centroid of all selected geometry.
    pub fn centroid(&self, scene: &Scene) -> glam::Vec3 {
        let mut sum = glam::Vec3::ZERO;
        let mut count = 0u32;

        for &(li, oi, fi) in &self.faces {
            if let Some(face) = scene.layers.get(li)
                .and_then(|l| l.objects.get(oi))
                .and_then(|o| o.faces.get(fi))
            {
                for p in &face.positions {
                    sum += *p;
                    count += 1;
                }
            }
        }

        for &(li, oi) in &self.objects {
            if let Some(object) = scene.layers.get(li).and_then(|l| l.objects.get(oi)) {
                for face in &object.faces {
                    for p in &face.positions {
                        sum += *p;
                        count += 1;
                    }
                }
            }
        }

        for &(li, oi, fi, vi) in &self.vertices {
            if let Some(pos) = scene.layers.get(li)
                .and_then(|l| l.objects.get(oi))
                .and_then(|o| o.faces.get(fi))
                .map(|f| f.positions[vi])
            {
                sum += pos;
                count += 1;
            }
        }

        if count > 0 { sum / count as f32 } else { glam::Vec3::ZERO }
    }
}

/// State for a direct vertex/face drag in the viewport.
pub struct VertexDrag {
    /// Constraint plane normal (perpendicular to camera).
    pub plane_normal: Vec3,
    /// Point on the constraint plane where drag started.
    pub start_world: Vec3,
    /// All vertices being dragged: (li, oi, fi, vi, original_position).
    pub targets: Vec<(usize, usize, usize, usize, Vec3)>,
    /// Accumulated delta applied so far.
    pub applied_delta: Vec3,
}

/// Active edit-mode state.
pub struct EditState {
    pub selection_level: SelectionLevel,
    pub gizmo_mode: GizmoMode,
    pub selection: Selection,
    /// Which gizmo axis the mouse is hovering over (for highlight).
    pub gizmo_hovered: GizmoAxis,
    /// Active gizmo drag operation (None when not dragging).
    pub gizmo_drag: Option<GizmoDrag>,
    /// Active direct vertex/face drag (None when not dragging).
    pub vertex_drag: Option<VertexDrag>,
}

impl EditState {
    pub fn new() -> Self {
        Self {
            selection_level: SelectionLevel::Face,
            gizmo_mode: GizmoMode::Translate,
            selection: Selection::default(),
            gizmo_hovered: GizmoAxis::None,
            gizmo_drag: None,
            vertex_drag: None,
        }
    }

    /// Marquee (drag box) selection: select all faces/objects with vertices inside the screen rect.
    pub fn marquee_select(
        &mut self,
        scene: &Scene,
        rect_min: Vec2,
        rect_max: Vec2,
        view_proj: Mat4,
        screen_size: Vec2,
        shift_held: bool,
    ) {
        if !shift_held {
            self.selection.clear();
        }

        let min_x = rect_min.x.min(rect_max.x);
        let max_x = rect_min.x.max(rect_max.x);
        let min_y = rect_min.y.min(rect_max.y);
        let max_y = rect_min.y.max(rect_max.y);

        for (li, layer) in scene.layers.iter().enumerate() {
            if !layer.visible {
                continue;
            }
            for (oi, object) in layer.objects.iter().enumerate() {
                match self.selection_level {
                    SelectionLevel::Object => {
                        let mut any_inside = false;
                        'obj_check: for face in &object.faces {
                            for &pos in &face.positions {
                                if let Some(sp) = project_to_screen(pos, view_proj, screen_size)
                                    && sp.x >= min_x && sp.x <= max_x && sp.y >= min_y && sp.y <= max_y
                                {
                                    any_inside = true;
                                    break 'obj_check;
                                }
                            }
                        }
                        if any_inside {
                            let entry = (li, oi);
                            if !self.selection.objects.contains(&entry) {
                                self.selection.objects.push(entry);
                            }
                        }
                    }
                    SelectionLevel::Face => {
                        for (fi, face) in object.faces.iter().enumerate() {
                            let any_inside = face.positions.iter().any(|&pos| {
                                project_to_screen(pos, view_proj, screen_size)
                                    .is_some_and(|sp| sp.x >= min_x && sp.x <= max_x && sp.y >= min_y && sp.y <= max_y)
                            });
                            if any_inside {
                                let entry = (li, oi, fi);
                                if !self.selection.faces.contains(&entry) {
                                    self.selection.faces.push(entry);
                                }
                            }
                        }
                    }
                    SelectionLevel::Edge => {
                        for (fi, face) in object.faces.iter().enumerate() {
                            for ei in 0..4 {
                                let a = face.positions[ei];
                                let b = face.positions[(ei + 1) % 4];
                                let a_inside = project_to_screen(a, view_proj, screen_size)
                                    .is_some_and(|sp| sp.x >= min_x && sp.x <= max_x && sp.y >= min_y && sp.y <= max_y);
                                let b_inside = project_to_screen(b, view_proj, screen_size)
                                    .is_some_and(|sp| sp.x >= min_x && sp.x <= max_x && sp.y >= min_y && sp.y <= max_y);
                                if a_inside && b_inside {
                                    let entry = (li, oi, fi, ei);
                                    if !self.selection.edges.contains(&entry) {
                                        self.selection.edges.push(entry);
                                    }
                                }
                            }
                        }
                    }
                    SelectionLevel::Vertex => {
                        for (fi, face) in object.faces.iter().enumerate() {
                            for (vi, &pos) in face.positions.iter().enumerate() {
                                if let Some(sp) = project_to_screen(pos, view_proj, screen_size)
                                    && sp.x >= min_x && sp.x <= max_x && sp.y >= min_y && sp.y <= max_y
                                {
                                    let entry = (li, oi, fi, vi);
                                    if !self.selection.vertices.contains(&entry) {
                                        self.selection.vertices.push(entry);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// Select all geometry in visible layers, respecting current selection level.
    pub fn select_all(&mut self, scene: &Scene) {
        self.selection.clear();
        for (li, layer) in scene.layers.iter().enumerate() {
            if !layer.visible { continue; }
            for (oi, object) in layer.objects.iter().enumerate() {
                match self.selection_level {
                    SelectionLevel::Object => {
                        self.selection.objects.push((li, oi));
                    }
                    SelectionLevel::Face => {
                        for fi in 0..object.faces.len() {
                            self.selection.faces.push((li, oi, fi));
                        }
                    }
                    SelectionLevel::Edge => {
                        for fi in 0..object.faces.len() {
                            for ei in 0..4 {
                                self.selection.edges.push((li, oi, fi, ei));
                            }
                        }
                    }
                    SelectionLevel::Vertex => {
                        for (fi, face) in object.faces.iter().enumerate() {
                            for vi in 0..face.positions.len() {
                                self.selection.vertices.push((li, oi, fi, vi));
                            }
                        }
                    }
                }
            }
        }
    }

    /// Invert the selection: select everything not currently selected, deselect what is.
    pub fn invert_selection(&mut self, scene: &Scene) {
        match self.selection_level {
            SelectionLevel::Object => {
                let mut all = Vec::new();
                for (li, layer) in scene.layers.iter().enumerate() {
                    if !layer.visible { continue; }
                    for oi in 0..layer.objects.len() {
                        all.push((li, oi));
                    }
                }
                let old = std::mem::take(&mut self.selection.objects);
                self.selection.objects = all.into_iter().filter(|e| !old.contains(e)).collect();
            }
            SelectionLevel::Face => {
                let mut all = Vec::new();
                for (li, layer) in scene.layers.iter().enumerate() {
                    if !layer.visible { continue; }
                    for (oi, object) in layer.objects.iter().enumerate() {
                        for fi in 0..object.faces.len() {
                            all.push((li, oi, fi));
                        }
                    }
                }
                let old = std::mem::take(&mut self.selection.faces);
                self.selection.faces = all.into_iter().filter(|e| !old.contains(e)).collect();
            }
            SelectionLevel::Edge => {
                let mut all = Vec::new();
                for (li, layer) in scene.layers.iter().enumerate() {
                    if !layer.visible { continue; }
                    for (oi, object) in layer.objects.iter().enumerate() {
                        for fi in 0..object.faces.len() {
                            for ei in 0..4 {
                                all.push((li, oi, fi, ei));
                            }
                        }
                    }
                }
                let old = std::mem::take(&mut self.selection.edges);
                self.selection.edges = all.into_iter().filter(|e| !old.contains(e)).collect();
            }
            SelectionLevel::Vertex => {
                let mut all = Vec::new();
                for (li, layer) in scene.layers.iter().enumerate() {
                    if !layer.visible { continue; }
                    for (oi, object) in layer.objects.iter().enumerate() {
                        for (fi, face) in object.faces.iter().enumerate() {
                            for vi in 0..face.positions.len() {
                                all.push((li, oi, fi, vi));
                            }
                        }
                    }
                }
                let old = std::mem::take(&mut self.selection.vertices);
                self.selection.vertices = all.into_iter().filter(|e| !old.contains(e)).collect();
            }
        }
    }

    /// Select all faces that share edges with currently selected faces.
    pub fn select_connected(&mut self, scene: &Scene) {
        if self.selection.faces.is_empty() { return; }

        let mut selected: std::collections::HashSet<(usize, usize, usize)> =
            self.selection.faces.iter().copied().collect();
        let mut frontier: Vec<(usize, usize, usize)> = self.selection.faces.clone();

        while let Some((li, oi, fi)) = frontier.pop() {
            let face = &scene.layers[li].objects[oi].faces[fi];
            // Check all other faces in the same object for shared edges
            for (ofi, other) in scene.layers[li].objects[oi].faces.iter().enumerate() {
                if selected.contains(&(li, oi, ofi)) { continue; }
                // Two faces share an edge if they have 2+ matching vertex positions
                let mut shared = 0;
                for p in &face.positions {
                    for op in &other.positions {
                        if (*p - *op).length_squared() < 1e-6 {
                            shared += 1;
                            break;
                        }
                    }
                }
                if shared >= 2 {
                    selected.insert((li, oi, ofi));
                    frontier.push((li, oi, ofi));
                }
            }
        }

        self.selection.faces = selected.into_iter().collect();
    }

    /// Select all faces whose normal faces toward the camera direction (within angle threshold).
    pub fn select_by_normal(&mut self, scene: &Scene, camera_forward: Vec3, threshold_degrees: f32) {
        let threshold_cos = threshold_degrees.to_radians().cos();
        self.selection.clear();
        for (li, layer) in scene.layers.iter().enumerate() {
            if !layer.visible { continue; }
            for (oi, object) in layer.objects.iter().enumerate() {
                for (fi, face) in object.faces.iter().enumerate() {
                    if face.hidden { continue; }
                    let n = face.normal();
                    // Face is "facing camera" if its normal points toward the camera
                    // (dot product of normal with -camera_forward > threshold)
                    if n.dot(-camera_forward) > threshold_cos {
                        self.selection.faces.push((li, oi, fi));
                    }
                }
            }
        }
    }

    /// Select all faces that overlap (same position within epsilon) with another face.
    pub fn select_overlapping(&mut self, scene: &Scene) {
        self.selection.clear();
        let eps = 1e-4;

        // Collect face centroids for fast overlap detection
        let mut face_data: Vec<(usize, usize, usize, Vec3)> = Vec::new();
        for (li, layer) in scene.layers.iter().enumerate() {
            if !layer.visible { continue; }
            for (oi, object) in layer.objects.iter().enumerate() {
                for (fi, face) in object.faces.iter().enumerate() {
                    if face.hidden { continue; }
                    let centroid = (face.positions[0] + face.positions[1] + face.positions[2] + face.positions[3]) * 0.25;
                    face_data.push((li, oi, fi, centroid));
                }
            }
        }

        let mut overlap_set = std::collections::HashSet::new();
        for i in 0..face_data.len() {
            for j in (i + 1)..face_data.len() {
                if (face_data[i].3 - face_data[j].3).length_squared() < eps {
                    overlap_set.insert((face_data[i].0, face_data[i].1, face_data[i].2));
                    overlap_set.insert((face_data[j].0, face_data[j].1, face_data[j].2));
                }
            }
        }

        self.selection.faces = overlap_set.into_iter().collect();
    }

    /// Select faces that use UVs matching the given tile UVs (within epsilon).
    pub fn select_by_uvs(&mut self, scene: &Scene, target_uvs: &[glam::Vec2; 4]) {
        self.selection.clear();
        let eps = 1e-4;
        for (li, layer) in scene.layers.iter().enumerate() {
            if !layer.visible { continue; }
            for (oi, object) in layer.objects.iter().enumerate() {
                for (fi, face) in object.faces.iter().enumerate() {
                    if face.hidden { continue; }
                    let matches = face.uvs.iter().zip(target_uvs.iter())
                        .all(|(a, b)| (*a - *b).length_squared() <= eps);
                    if matches {
                        self.selection.faces.push((li, oi, fi));
                    }
                }
            }
        }
    }

    /// Select edge loop: follow connected edges where each intermediate vertex connects exactly 2 edges.
    pub fn select_edge_loop(&mut self, scene: &Scene) {
        if self.selection.edges.is_empty() { return; }

        // Build edge-to-face adjacency for the same object
        let &(li, oi, fi, ei) = &self.selection.edges[0];
        let object = match scene.layers.get(li).and_then(|l| l.objects.get(oi)) {
            Some(o) => o,
            None => return,
        };

        // Build a map from edge (as sorted vertex pair by position) to face+edge_index
        let eps = 1e-5;
        let mut positions: Vec<Vec3> = Vec::new();
        let mut pos_to_idx = |p: Vec3| -> usize {
            for (i, &existing) in positions.iter().enumerate() {
                if (existing - p).length_squared() < eps {
                    return i;
                }
            }
            positions.push(p);
            positions.len() - 1
        };

        // Map: vertex_index -> list of (face_idx, edge_idx, other_vertex_index)
        let mut vert_edges: std::collections::HashMap<usize, Vec<(usize, usize, usize)>> = std::collections::HashMap::new();

        for (face_idx, face) in object.faces.iter().enumerate() {
            for edge_idx in 0..4 {
                let a_idx = pos_to_idx(face.positions[edge_idx]);
                let b_idx = pos_to_idx(face.positions[(edge_idx + 1) % 4]);
                vert_edges.entry(a_idx).or_default().push((face_idx, edge_idx, b_idx));
                vert_edges.entry(b_idx).or_default().push((face_idx, edge_idx, a_idx));
            }
        }

        // Start from the seed edge, walk in both directions
        let seed_face = &object.faces[fi];
        let start_a = pos_to_idx(seed_face.positions[ei]);
        let start_b = pos_to_idx(seed_face.positions[(ei + 1) % 4]);

        let mut selected_edges = std::collections::HashSet::new();
        selected_edges.insert((li, oi, fi, ei));

        // Walk from start_b forward
        let mut current = start_b;
        let mut prev = start_a;
        for _ in 0..1000 {
            // Find edges connected to current vertex (excluding the one we came from)
            let edges = match vert_edges.get(&current) {
                Some(e) => e,
                None => break,
            };
            let next_edges: Vec<_> = edges.iter()
                .filter(|&&(_, _, other)| other != prev)
                .copied()
                .collect();
            // Continue only if there's exactly one continuation (clean edge loop)
            if next_edges.len() != 1 { break; }
            let (nf, ne, next_vert) = next_edges[0];
            selected_edges.insert((li, oi, nf, ne));
            if next_vert == start_a { break; } // Completed the loop
            prev = current;
            current = next_vert;
        }

        // Walk from start_a backward
        let mut current = start_a;
        let mut prev = start_b;
        for _ in 0..1000 {
            let edges = match vert_edges.get(&current) {
                Some(e) => e,
                None => break,
            };
            let next_edges: Vec<_> = edges.iter()
                .filter(|&&(_, _, other)| other != prev)
                .copied()
                .collect();
            if next_edges.len() != 1 { break; }
            let (nf, ne, next_vert) = next_edges[0];
            if selected_edges.contains(&(li, oi, nf, ne)) { break; } // Already visited
            selected_edges.insert((li, oi, nf, ne));
            prev = current;
            current = next_vert;
        }

        self.selection.edges = selected_edges.into_iter().collect();
    }

    /// Select faces connected to currently selected vertices.
    pub fn select_faces_from_vertices(&mut self, scene: &Scene) {
        if self.selection.vertices.is_empty() { return; }

        let mut vertex_positions: Vec<Vec3> = Vec::new();
        for &(li, oi, fi, vi) in &self.selection.vertices {
            if let Some(face) = scene.layers.get(li)
                .and_then(|l| l.objects.get(oi))
                .and_then(|o| o.faces.get(fi))
            {
                vertex_positions.push(face.positions[vi]);
            }
        }

        self.selection.faces.clear();
        let eps = 1e-5;
        for (li, layer) in scene.layers.iter().enumerate() {
            if !layer.visible { continue; }
            for (oi, object) in layer.objects.iter().enumerate() {
                for (fi, face) in object.faces.iter().enumerate() {
                    if face.hidden { continue; }
                    let has_selected_vert = face.positions.iter().any(|p| {
                        vertex_positions.iter().any(|vp| (*p - *vp).length_squared() < eps)
                    });
                    if has_selected_vert {
                        let entry = (li, oi, fi);
                        if !self.selection.faces.contains(&entry) {
                            self.selection.faces.push(entry);
                        }
                    }
                }
            }
        }
    }

    /// Handle a left-click in edit mode — select the face/object under the cursor.
    pub fn handle_click(&mut self, ray: &Ray, scene: &Scene, shift_held: bool) {
        let hit = picking::pick_face(ray, scene);

        if !shift_held {
            self.selection.clear();
        }

        if let Some(hit) = hit {
            match self.selection_level {
                SelectionLevel::Object => {
                    let entry = (hit.layer_index, hit.object_index);
                    if !self.selection.objects.contains(&entry) {
                        self.selection.objects.push(entry);
                    }
                }
                SelectionLevel::Face => {
                    let entry = (hit.layer_index, hit.object_index, hit.face_index);
                    if !self.selection.faces.contains(&entry) {
                        self.selection.faces.push(entry);
                    }
                }
                SelectionLevel::Vertex => {
                    // Select the closest vertex of the hit face
                    let face = &scene.layers[hit.layer_index].objects[hit.object_index].faces[hit.face_index];
                    let closest_vi = face.positions.iter().enumerate()
                        .min_by(|(_, a), (_, b)| {
                            let da = a.distance(hit.position);
                            let db = b.distance(hit.position);
                            da.partial_cmp(&db).unwrap()
                        })
                        .map(|(i, _)| i)
                        .unwrap_or(0);
                    let entry = (hit.layer_index, hit.object_index, hit.face_index, closest_vi);
                    if !self.selection.vertices.contains(&entry) {
                        self.selection.vertices.push(entry);
                    }
                }
                SelectionLevel::Edge => {
                    // Select the closest edge of the hit face
                    let face = &scene.layers[hit.layer_index].objects[hit.object_index].faces[hit.face_index];
                    let closest_edge = (0..4usize)
                        .min_by(|&i, &j| {
                            let mid_i = (face.positions[i] + face.positions[(i + 1) % 4]) * 0.5;
                            let mid_j = (face.positions[j] + face.positions[(j + 1) % 4]) * 0.5;
                            let di = mid_i.distance_squared(hit.position);
                            let dj = mid_j.distance_squared(hit.position);
                            di.partial_cmp(&dj).unwrap()
                        })
                        .unwrap_or(0);
                    let entry = (hit.layer_index, hit.object_index, hit.face_index, closest_edge);
                    if !self.selection.edges.contains(&entry) {
                        self.selection.edges.push(entry);
                    }
                }
            }
        }
    }

}
