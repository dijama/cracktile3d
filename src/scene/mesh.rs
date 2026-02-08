use glam::{Vec2, Vec3, Vec4};
use serde::{Serialize, Deserialize};
use crate::render::Vertex;

/// A single quad face (4 vertices, 2 triangles).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Face {
    pub positions: [Vec3; 4],
    pub uvs: [Vec2; 4],
    pub colors: [Vec4; 4],
    #[serde(default)]
    pub hidden: bool,
}

impl Face {
    /// Create a face from a position, normal, and tile UVs.
    /// The quad is axis-aligned on the plane perpendicular to `normal`.
    pub fn new_quad(center: Vec3, normal: Vec3, half_size: f32, uvs: [Vec2; 4]) -> Self {
        let (right, up) = tangent_basis(normal);
        let r = right * half_size;
        let u = up * half_size;

        Self {
            positions: [
                center - r - u,
                center + r - u,
                center + r + u,
                center - r + u,
            ],
            uvs,
            colors: [Vec4::ONE; 4],
            hidden: false,
        }
    }

    /// Create a rectangular quad (different width/height) for multi-tile placement.
    pub fn new_rect_quad(center: Vec3, normal: Vec3, half_w: f32, half_h: f32, uvs: [Vec2; 4]) -> Self {
        let (right, up) = tangent_basis(normal);
        let r = right * half_w;
        let u = up * half_h;

        Self {
            positions: [
                center - r - u,
                center + r - u,
                center + r + u,
                center - r + u,
            ],
            uvs,
            colors: [Vec4::ONE; 4],
            hidden: false,
        }
    }

    pub fn vertices(&self) -> [Vertex; 4] {
        std::array::from_fn(|i| Vertex {
            position: self.positions[i].into(),
            uv: self.uvs[i].into(),
            color: self.colors[i].into(),
        })
    }

    /// The two triangles forming this quad (indices 0,1,2 and 0,2,3).
    pub fn indices(base: u32) -> [u32; 6] {
        [base, base + 1, base + 2, base, base + 2, base + 3]
    }

    pub fn normal(&self) -> Vec3 {
        let e1 = self.positions[1] - self.positions[0];
        let e2 = self.positions[3] - self.positions[0];
        e1.cross(e2).normalize()
    }
}

/// Compute a tangent basis (right, up) for a given normal direction.
fn tangent_basis(normal: Vec3) -> (Vec3, Vec3) {
    let n = normal.normalize();
    let reference = if n.y.abs() > 0.9 { Vec3::Z } else { Vec3::Y };
    let right = reference.cross(n).normalize();
    let up = n.cross(right).normalize();
    (right, up)
}
