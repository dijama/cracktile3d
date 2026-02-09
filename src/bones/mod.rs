//! Skeletal animation: bone hierarchy, vertex weights, and skeleton management.

use glam::{Vec3, Quat, Mat4};
use serde::{Serialize, Deserialize};

/// A single bone in the skeleton hierarchy.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Bone {
    pub name: String,
    /// Index of parent bone (None for root bones).
    pub parent: Option<usize>,
    /// Rest position of the bone head (base) in world space.
    pub head: Vec3,
    /// Rest position of the bone tail (tip) in world space.
    pub tail: Vec3,
    /// Pose rotation relative to rest pose.
    #[serde(default = "default_quat")]
    pub pose_rotation: Quat,
    /// Pose translation relative to rest pose.
    #[serde(default)]
    pub pose_translation: Vec3,
    /// Whether this bone is selected in the UI.
    #[serde(skip)]
    pub selected: bool,
}

fn default_quat() -> Quat { Quat::IDENTITY }

impl Bone {
    pub fn new(name: String, head: Vec3, tail: Vec3, parent: Option<usize>) -> Self {
        Self {
            name,
            parent,
            head,
            tail,
            pose_rotation: Quat::IDENTITY,
            pose_translation: Vec3::ZERO,
            selected: false,
        }
    }

    /// Length of this bone (head to tail distance).
    pub fn length(&self) -> f32 {
        (self.tail - self.head).length()
    }

    /// Direction from head to tail (normalized).
    pub fn direction(&self) -> Vec3 {
        (self.tail - self.head).normalize_or_zero()
    }

    /// Compute the rest-pose local transform (head as origin, pointing toward tail).
    pub fn rest_matrix(&self) -> Mat4 {
        Mat4::from_translation(self.head)
    }

    /// Compute the posed transform: rest + pose_rotation around head + translation.
    pub fn posed_matrix(&self) -> Mat4 {
        let t = Mat4::from_translation(self.head + self.pose_translation);
        let r = Mat4::from_quat(self.pose_rotation);
        t * r
    }

    /// Get the posed head position.
    pub fn posed_head(&self) -> Vec3 {
        self.head + self.pose_translation
    }

    /// Get the posed tail position.
    pub fn posed_tail(&self) -> Vec3 {
        let local_tail = self.tail - self.head;
        self.posed_head() + self.pose_rotation * local_tail
    }
}

/// A skeleton: an ordered list of bones forming a hierarchy.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Skeleton {
    pub bones: Vec<Bone>,
}

impl Skeleton {
    pub fn new() -> Self {
        Self { bones: Vec::new() }
    }

    /// Add a bone and return its index.
    pub fn add_bone(&mut self, bone: Bone) -> usize {
        let idx = self.bones.len();
        self.bones.push(bone);
        idx
    }

    /// Find children of a given bone index.
    pub fn children_of(&self, parent_idx: usize) -> Vec<usize> {
        self.bones.iter().enumerate()
            .filter(|(_, b)| b.parent == Some(parent_idx))
            .map(|(i, _)| i)
            .collect()
    }

    /// Find root bones (bones with no parent).
    pub fn roots(&self) -> Vec<usize> {
        self.bones.iter().enumerate()
            .filter(|(_, b)| b.parent.is_none())
            .map(|(i, _)| i)
            .collect()
    }

    /// Select a bone by index, optionally adding to selection.
    pub fn select_bone(&mut self, idx: usize, add_to_selection: bool) {
        if !add_to_selection {
            for bone in &mut self.bones {
                bone.selected = false;
            }
        }
        if let Some(bone) = self.bones.get_mut(idx) {
            bone.selected = true;
        }
    }

    /// Deselect all bones.
    pub fn deselect_all(&mut self) {
        for bone in &mut self.bones {
            bone.selected = false;
        }
    }

    /// Get indices of all selected bones.
    pub fn selected_indices(&self) -> Vec<usize> {
        self.bones.iter().enumerate()
            .filter(|(_, b)| b.selected)
            .map(|(i, _)| i)
            .collect()
    }

    /// Generate line segments for rendering the skeleton (head→tail for each bone).
    /// Returns pairs of (start, end) positions.
    pub fn render_lines(&self) -> Vec<(Vec3, Vec3)> {
        self.bones.iter().map(|b| (b.posed_head(), b.posed_tail())).collect()
    }

    /// Generate line segments for selected bones only.
    pub fn selected_render_lines(&self) -> Vec<(Vec3, Vec3)> {
        self.bones.iter()
            .filter(|b| b.selected)
            .map(|b| (b.posed_head(), b.posed_tail()))
            .collect()
    }

    /// Find the bone closest to a world-space point (by distance to the bone line segment).
    /// Returns (bone_index, distance).
    pub fn pick_bone(&self, point: Vec3, max_dist: f32) -> Option<(usize, f32)> {
        let mut best: Option<(usize, f32)> = None;
        for (i, bone) in self.bones.iter().enumerate() {
            let d = point_to_segment_distance(point, bone.posed_head(), bone.posed_tail());
            if d < max_dist
                && (best.is_none() || d < best.unwrap().1)
            {
                best = Some((i, d));
            }
        }
        best
    }
}

/// Compute the distance from a point to a line segment.
fn point_to_segment_distance(p: Vec3, a: Vec3, b: Vec3) -> f32 {
    let ab = b - a;
    let ap = p - a;
    let len_sq = ab.length_squared();
    if len_sq < 1e-10 {
        return ap.length();
    }
    let t = (ap.dot(ab) / len_sq).clamp(0.0, 1.0);
    let closest = a + ab * t;
    (p - closest).length()
}
