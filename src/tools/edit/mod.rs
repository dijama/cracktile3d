use glam::{Mat4, Vec2};
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

/// Active edit-mode state.
pub struct EditState {
    pub selection_level: SelectionLevel,
    pub gizmo_mode: GizmoMode,
    pub selection: Selection,
}

impl EditState {
    pub fn new() -> Self {
        Self {
            selection_level: SelectionLevel::Face,
            gizmo_mode: GizmoMode::Translate,
            selection: Selection::default(),
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
