use glam::Vec3;

use crate::scene::Scene;
use crate::util::picking::{self, HitResult, Ray};

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
}

impl Selection {
    pub fn clear(&mut self) {
        self.objects.clear();
        self.faces.clear();
        self.vertices.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.objects.is_empty() && self.faces.is_empty() && self.vertices.is_empty()
    }
}

/// Active edit-mode state.
pub struct EditState {
    pub selection_level: SelectionLevel,
    pub gizmo_mode: GizmoMode,
    pub selection: Selection,
    /// Accumulated transform during an active drag
    drag_start: Option<Vec3>,
}

impl EditState {
    pub fn new() -> Self {
        Self {
            selection_level: SelectionLevel::Face,
            gizmo_mode: GizmoMode::Translate,
            selection: Selection::default(),
            drag_start: None,
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
                    // For now, treat edge selection as face selection
                    let entry = (hit.layer_index, hit.object_index, hit.face_index);
                    if !self.selection.faces.contains(&entry) {
                        self.selection.faces.push(entry);
                    }
                }
            }
        }
    }

    /// Translate all selected faces by a delta vector.
    pub fn translate_selection(&self, scene: &mut Scene, delta: Vec3, device: &wgpu::Device) {
        let mut rebuild_objects: std::collections::HashSet<(usize, usize)> = std::collections::HashSet::new();

        for &(li, oi, fi) in &self.selection.faces {
            let face = &mut scene.layers[li].objects[oi].faces[fi];
            for pos in &mut face.positions {
                *pos += delta;
            }
            rebuild_objects.insert((li, oi));
        }

        for &(li, oi) in &self.selection.objects {
            let object = &mut scene.layers[li].objects[oi];
            for face in &mut object.faces {
                for pos in &mut face.positions {
                    *pos += delta;
                }
            }
            rebuild_objects.insert((li, oi));
        }

        for &(li, oi, fi, vi) in &self.selection.vertices {
            scene.layers[li].objects[oi].faces[fi].positions[vi] += delta;
            rebuild_objects.insert((li, oi));
        }

        for (li, oi) in rebuild_objects {
            scene.layers[li].objects[oi].rebuild_gpu_mesh(device);
        }
    }

    /// Delete all selected faces.
    pub fn delete_selection(&mut self, scene: &mut Scene, device: &wgpu::Device) {
        // Sort face indices in reverse so removal doesn't invalidate earlier indices
        let mut faces = self.selection.faces.clone();
        faces.sort_by(|a, b| b.2.cmp(&a.2));

        let mut rebuild_objects: std::collections::HashSet<(usize, usize)> = std::collections::HashSet::new();

        for (li, oi, fi) in faces {
            scene.layers[li].objects[oi].faces.remove(fi);
            rebuild_objects.insert((li, oi));
        }

        // Delete entire objects if selected at object level
        for &(li, oi) in self.selection.objects.iter().rev() {
            scene.layers[li].objects.remove(oi);
        }

        for (li, oi) in rebuild_objects {
            if oi < scene.layers[li].objects.len() {
                scene.layers[li].objects[oi].rebuild_gpu_mesh(device);
            }
        }

        self.selection.clear();
    }
}
