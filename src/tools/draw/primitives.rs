use glam::{Vec2, Vec3, Vec4};
use crate::scene::mesh::Face;

/// Generate a box (6 quad faces) centered at `center` with `half_size` extents.
pub fn generate_box(center: Vec3, half_size: Vec3, uvs: [Vec2; 4]) -> Vec<Face> {
    let h = half_size;
    vec![
        Face::new_quad(center + Vec3::new(0.0, h.y, 0.0), Vec3::Y, h.x, uvs),
        Face::new_quad(center - Vec3::new(0.0, h.y, 0.0), -Vec3::Y, h.x, uvs),
        Face::new_quad(center + Vec3::new(0.0, 0.0, h.z), Vec3::Z, h.x, uvs),
        Face::new_quad(center - Vec3::new(0.0, 0.0, h.z), -Vec3::Z, h.x, uvs),
        Face::new_quad(center + Vec3::new(h.x, 0.0, 0.0), Vec3::X, h.z, uvs),
        Face::new_quad(center - Vec3::new(h.x, 0.0, 0.0), -Vec3::X, h.z, uvs),
    ]
}

/// Generate a cylinder with `segments` side quads plus top/bottom cap quads.
pub fn generate_cylinder(center: Vec3, radius: f32, height: f32, segments: usize, uvs: [Vec2; 4]) -> Vec<Face> {
    let half_h = height * 0.5;
    let mut faces = Vec::new();

    for i in 0..segments {
        let a0 = std::f32::consts::TAU * (i as f32) / (segments as f32);
        let a1 = std::f32::consts::TAU * ((i + 1) as f32) / (segments as f32);
        let (s0, c0) = (a0.sin(), a0.cos());
        let (s1, c1) = (a1.sin(), a1.cos());

        // Side quad
        let bl = center + Vec3::new(radius * s0, -half_h, radius * c0);
        let br = center + Vec3::new(radius * s1, -half_h, radius * c1);
        let tr = center + Vec3::new(radius * s1, half_h, radius * c1);
        let tl = center + Vec3::new(radius * s0, half_h, radius * c0);
        faces.push(Face { positions: [bl, br, tr, tl], uvs, colors: [Vec4::ONE; 4], hidden: false });

        // Top cap quad (triangle as degenerate quad: center, p0, p1, center)
        let tc = center + Vec3::new(0.0, half_h, 0.0);
        let t0 = center + Vec3::new(radius * s0, half_h, radius * c0);
        let t1 = center + Vec3::new(radius * s1, half_h, radius * c1);
        faces.push(Face { positions: [tc, t0, t1, tc], uvs, colors: [Vec4::ONE; 4], hidden: false });

        // Bottom cap quad (triangle as degenerate quad)
        let bc = center + Vec3::new(0.0, -half_h, 0.0);
        let b0 = center + Vec3::new(radius * s0, -half_h, radius * c0);
        let b1 = center + Vec3::new(radius * s1, -half_h, radius * c1);
        faces.push(Face { positions: [bc, b1, b0, bc], uvs, colors: [Vec4::ONE; 4], hidden: false });
    }

    faces
}

/// Generate a cone with `segments` side triangles (as degenerate quads) and a bottom cap.
pub fn generate_cone(center: Vec3, radius: f32, height: f32, segments: usize, uvs: [Vec2; 4]) -> Vec<Face> {
    let half_h = height * 0.5;
    let apex = center + Vec3::new(0.0, half_h, 0.0);
    let mut faces = Vec::new();

    for i in 0..segments {
        let a0 = std::f32::consts::TAU * (i as f32) / (segments as f32);
        let a1 = std::f32::consts::TAU * ((i + 1) as f32) / (segments as f32);
        let (s0, c0) = (a0.sin(), a0.cos());
        let (s1, c1) = (a1.sin(), a1.cos());

        // Side triangle (degenerate quad: apex shared at positions[2] and [3])
        let b0 = center + Vec3::new(radius * s0, -half_h, radius * c0);
        let b1 = center + Vec3::new(radius * s1, -half_h, radius * c1);
        faces.push(Face { positions: [b0, b1, apex, apex], uvs, colors: [Vec4::ONE; 4], hidden: false });

        // Bottom cap
        let bc = center + Vec3::new(0.0, -half_h, 0.0);
        faces.push(Face { positions: [bc, b1, b0, bc], uvs, colors: [Vec4::ONE; 4], hidden: false });
    }

    faces
}

/// Generate a UV-sphere tessellated into quads.
pub fn generate_sphere(center: Vec3, radius: f32, rings: usize, segments: usize, uvs: [Vec2; 4]) -> Vec<Face> {
    let mut faces = Vec::new();

    for ring in 0..rings {
        let theta0 = std::f32::consts::PI * (ring as f32) / (rings as f32);
        let theta1 = std::f32::consts::PI * ((ring + 1) as f32) / (rings as f32);
        let (st0, ct0) = (theta0.sin(), theta0.cos());
        let (st1, ct1) = (theta1.sin(), theta1.cos());

        for seg in 0..segments {
            let phi0 = std::f32::consts::TAU * (seg as f32) / (segments as f32);
            let phi1 = std::f32::consts::TAU * ((seg + 1) as f32) / (segments as f32);
            let (sp0, cp0) = (phi0.sin(), phi0.cos());
            let (sp1, cp1) = (phi1.sin(), phi1.cos());

            let p00 = center + radius * Vec3::new(st0 * sp0, ct0, st0 * cp0);
            let p10 = center + radius * Vec3::new(st1 * sp0, ct1, st1 * cp0);
            let p11 = center + radius * Vec3::new(st1 * sp1, ct1, st1 * cp1);
            let p01 = center + radius * Vec3::new(st0 * sp1, ct0, st0 * cp1);

            faces.push(Face {
                positions: [p00, p10, p11, p01],
                uvs,
                colors: [Vec4::ONE; 4],
                hidden: false,
            });
        }
    }

    faces
}

/// Generate a wedge (triangular prism): 5 faces (2 triangular ends + 3 rectangular sides).
pub fn generate_wedge(center: Vec3, half_size: Vec3, uvs: [Vec2; 4]) -> Vec<Face> {
    let h = half_size;

    // Wedge vertices:
    //   Bottom quad: 4 corners at y = -h.y
    //   Top edge: 2 points at y = +h.y (only the front edge reaches the top)
    let bl_f = center + Vec3::new(-h.x, -h.y, -h.z); // bottom-left-front
    let br_f = center + Vec3::new(h.x, -h.y, -h.z);  // bottom-right-front
    let bl_b = center + Vec3::new(-h.x, -h.y, h.z);   // bottom-left-back
    let br_b = center + Vec3::new(h.x, -h.y, h.z);    // bottom-right-back
    let tl = center + Vec3::new(-h.x, h.y, -h.z);     // top-left (front edge)
    let tr = center + Vec3::new(h.x, h.y, -h.z);      // top-right (front edge)

    vec![
        // Bottom face
        Face { positions: [bl_b, br_b, br_f, bl_f], uvs, colors: [Vec4::ONE; 4], hidden: false },
        // Front face (vertical)
        Face { positions: [bl_f, br_f, tr, tl], uvs, colors: [Vec4::ONE; 4], hidden: false },
        // Back/slope face
        Face { positions: [br_b, bl_b, tl, tr], uvs, colors: [Vec4::ONE; 4], hidden: false },
        // Left triangular end (degenerate quad)
        Face { positions: [bl_b, bl_f, tl, tl], uvs, colors: [Vec4::ONE; 4], hidden: false },
        // Right triangular end (degenerate quad)
        Face { positions: [br_f, br_b, tr, tr], uvs, colors: [Vec4::ONE; 4], hidden: false },
    ]
}
