use glam::{Mat4, Vec2, Vec3, Vec4Swizzles};

/// A ray in 3D space with origin and direction.
#[derive(Debug, Clone, Copy)]
pub struct Ray {
    pub origin: Vec3,
    pub direction: Vec3,
}

/// Result of a ray hitting a face.
#[derive(Debug, Clone)]
pub struct HitResult {
    pub distance: f32,
    pub position: Vec3,
    pub normal: Vec3,
    pub layer_index: usize,
    pub object_index: usize,
    pub face_index: usize,
}

impl Ray {
    /// Create a ray from screen coordinates (pixels) through the camera.
    /// `screen_pos` is in pixels from top-left, `screen_size` is viewport width/height.
    pub fn from_screen(
        screen_pos: Vec2,
        screen_size: Vec2,
        view_proj: Mat4,
    ) -> Self {
        // Convert screen coords to NDC (-1..1)
        let ndc_x = (2.0 * screen_pos.x / screen_size.x) - 1.0;
        let ndc_y = 1.0 - (2.0 * screen_pos.y / screen_size.y); // Y is flipped

        let inv_vp = view_proj.inverse();

        let near_point = inv_vp.project_point3(Vec3::new(ndc_x, ndc_y, -1.0));
        let far_point = inv_vp.project_point3(Vec3::new(ndc_x, ndc_y, 1.0));

        let direction = (far_point - near_point).normalize();

        Self {
            origin: near_point,
            direction,
        }
    }

    /// Intersect ray with a triangle (Möller–Trumbore algorithm).
    /// Returns distance along ray if hit, None if miss.
    pub fn intersect_triangle(&self, v0: Vec3, v1: Vec3, v2: Vec3) -> Option<f32> {
        let edge1 = v1 - v0;
        let edge2 = v2 - v0;
        let h = self.direction.cross(edge2);
        let a = edge1.dot(h);

        if a.abs() < 1e-7 {
            return None; // Parallel to triangle
        }

        let f = 1.0 / a;
        let s = self.origin - v0;
        let u = f * s.dot(h);

        if !(0.0..=1.0).contains(&u) {
            return None;
        }

        let q = s.cross(edge1);
        let v = f * self.direction.dot(q);

        if v < 0.0 || u + v > 1.0 {
            return None;
        }

        let t = f * edge2.dot(q);
        if t > 1e-7 {
            Some(t)
        } else {
            None
        }
    }

    /// Intersect ray with a quad (two triangles: 0-1-2 and 0-2-3).
    pub fn intersect_quad(&self, positions: &[Vec3; 4]) -> Option<f32> {
        let t1 = self.intersect_triangle(positions[0], positions[1], positions[2]);
        let t2 = self.intersect_triangle(positions[0], positions[2], positions[3]);

        match (t1, t2) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        }
    }

    /// Intersect ray with an infinite plane defined by a point and normal.
    /// Returns distance along ray if hit, None if parallel.
    pub fn intersect_plane(&self, plane_point: Vec3, plane_normal: Vec3) -> Option<f32> {
        let denom = plane_normal.dot(self.direction);
        if denom.abs() < 1e-7 {
            return None;
        }
        let t = (plane_point - self.origin).dot(plane_normal) / denom;
        if t > 0.0 { Some(t) } else { None }
    }

    pub fn point_at(&self, t: f32) -> Vec3 {
        self.origin + self.direction * t
    }
}

/// Project a 3D point to 2D screen coordinates.
/// Returns None if the point is behind the camera.
pub fn project_to_screen(pos: Vec3, view_proj: Mat4, screen_size: Vec2) -> Option<Vec2> {
    let clip = view_proj * pos.extend(1.0);
    if clip.w <= 0.0 {
        return None;
    }
    let ndc = clip.xyz() / clip.w;
    Some(Vec2::new(
        (ndc.x + 1.0) * 0.5 * screen_size.x,
        (1.0 - ndc.y) * 0.5 * screen_size.y,
    ))
}

/// Pick the closest face in the scene hit by a screen-space ray.
/// When `cull_backfaces` is true, faces whose normals point away from the camera are skipped.
pub fn pick_face(
    ray: &Ray,
    scene: &crate::scene::Scene,
) -> Option<HitResult> {
    pick_face_ex(ray, scene, false)
}

/// Pick the closest front-facing face (back-face culled).
/// Use this for Draw mode placement to avoid picking invisible faces.
pub fn pick_face_culled(
    ray: &Ray,
    scene: &crate::scene::Scene,
) -> Option<HitResult> {
    pick_face_ex(ray, scene, true)
}

fn pick_face_ex(
    ray: &Ray,
    scene: &crate::scene::Scene,
    cull_backfaces: bool,
) -> Option<HitResult> {
    let mut closest: Option<HitResult> = None;

    for (li, layer) in scene.layers.iter().enumerate() {
        if !layer.visible {
            continue;
        }
        for (oi, object) in layer.objects.iter().enumerate() {
            for (fi, face) in object.faces.iter().enumerate() {
                if face.hidden { continue; }
                let normal = face.normal();
                // Skip back-facing faces (normal points away from camera)
                if cull_backfaces && normal.dot(ray.direction) > 0.0 {
                    continue;
                }
                if let Some(t) = ray.intersect_quad(&face.positions) {
                    let dominated = closest.as_ref().is_some_and(|c| c.distance <= t);
                    if !dominated {
                        closest = Some(HitResult {
                            distance: t,
                            position: ray.point_at(t),
                            normal,
                            layer_index: li,
                            object_index: oi,
                            face_index: fi,
                        });
                    }
                }
            }
        }
    }

    closest
}
