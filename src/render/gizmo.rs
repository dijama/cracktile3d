use glam::{Mat4, Vec2, Vec3};

use crate::render::vertex::LineVertex;
use crate::tools::edit::GizmoMode;
use crate::util::picking::{project_to_screen, Ray};

// Axis colors: X=Red, Y=Green, Z=Blue
const AXIS_COLORS: [[f32; 4]; 3] = [
    [1.0, 0.2, 0.2, 1.0],
    [0.2, 1.0, 0.2, 1.0],
    [0.3, 0.5, 1.0, 1.0],
];
const HIGHLIGHT_COLOR: [f32; 4] = [1.0, 1.0, 0.3, 1.0];

/// Which gizmo axis or plane the user is interacting with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GizmoAxis {
    None,
    X,
    Y,
    Z,
    XY,
    XZ,
    YZ,
}

impl GizmoAxis {
    /// Returns the world-space direction(s) for this axis/plane.
    pub fn direction(self) -> Vec3 {
        match self {
            GizmoAxis::X => Vec3::X,
            GizmoAxis::Y => Vec3::Y,
            GizmoAxis::Z => Vec3::Z,
            _ => Vec3::ZERO,
        }
    }
}

/// State of an active gizmo drag operation.
pub struct GizmoDrag {
    pub axis: GizmoAxis,
    /// World position on the constraint where the drag started.
    pub start_point: Vec3,
    /// Selection centroid at drag start.
    pub origin: Vec3,
    /// Accumulated delta applied so far (for live preview undo).
    pub applied_delta: Vec3,
    /// For rotation: start angle.
    pub start_angle: f32,
    /// For rotation: accumulated angle applied so far.
    pub applied_angle: f32,
    /// For scale: start distance from origin.
    pub start_distance: f32,
    /// For scale: accumulated scale applied so far.
    pub applied_scale: Vec3,
}

impl GizmoDrag {
    pub fn new(axis: GizmoAxis, start_point: Vec3, origin: Vec3) -> Self {
        Self {
            axis,
            start_point,
            origin,
            applied_delta: Vec3::ZERO,
            start_angle: 0.0,
            applied_angle: 0.0,
            start_distance: 1.0,
            applied_scale: Vec3::ONE,
        }
    }
}

/// Compute the gizmo visual scale so it appears constant on screen.
pub fn gizmo_scale(center: Vec3, camera_pos: Vec3) -> f32 {
    let dist = center.distance(camera_pos);
    dist * 0.15
}

/// Generate line vertices for the 3D gizmo at the given center.
pub fn build_gizmo_lines(
    center: Vec3,
    scale: f32,
    mode: GizmoMode,
    hovered: GizmoAxis,
    active: GizmoAxis,
) -> Vec<LineVertex> {
    let mut verts = Vec::new();
    let axes = [Vec3::X, Vec3::Y, Vec3::Z];

    match mode {
        GizmoMode::Translate => {
            for (i, &axis) in axes.iter().enumerate() {
                let color = axis_color(i, hovered, active);
                let tip = center + axis * scale;

                // Shaft
                verts.push(lv(center, color));
                verts.push(lv(tip, color));

                // Arrow head cone
                let head_len = scale * 0.2;
                let head_rad = scale * 0.06;
                let base = center + axis * (scale - head_len);
                let (p1, p2) = perpendiculars(axis);
                let offsets = [p1 * head_rad, -p1 * head_rad, p2 * head_rad, -p2 * head_rad];
                for off in &offsets {
                    verts.push(lv(tip, color));
                    verts.push(lv(base + *off, color));
                }
                for j in 0..4 {
                    verts.push(lv(base + offsets[j], color));
                    verts.push(lv(base + offsets[(j + 1) % 4], color));
                }
            }

            // Plane handles: small squares at 1/3 scale along pairs of axes
            let psize = scale * 0.12;
            let poff = scale * 0.25;
            let planes = [
                (0, 1, GizmoAxis::XY),
                (0, 2, GizmoAxis::XZ),
                (1, 2, GizmoAxis::YZ),
            ];
            for &(ai, bi, pa) in &planes {
                let color = plane_color(pa, hovered, active);
                let a = axes[ai];
                let b = axes[bi];
                let corners = [
                    center + a * poff + b * poff,
                    center + a * (poff + psize) + b * poff,
                    center + a * (poff + psize) + b * (poff + psize),
                    center + a * poff + b * (poff + psize),
                ];
                for j in 0..4 {
                    verts.push(lv(corners[j], color));
                    verts.push(lv(corners[(j + 1) % 4], color));
                }
            }
        }
        GizmoMode::Rotate => {
            let segments = 48;
            let radius = scale * 0.85;
            for (i, &axis) in axes.iter().enumerate() {
                let color = axis_color(i, hovered, active);
                let (p1, p2) = perpendiculars(axis);
                for s in 0..segments {
                    let a0 = std::f32::consts::TAU * s as f32 / segments as f32;
                    let a1 = std::f32::consts::TAU * (s + 1) as f32 / segments as f32;
                    let pt0 = center + (p1 * a0.cos() + p2 * a0.sin()) * radius;
                    let pt1 = center + (p1 * a1.cos() + p2 * a1.sin()) * radius;
                    verts.push(lv(pt0, color));
                    verts.push(lv(pt1, color));
                }
            }
        }
        GizmoMode::Scale => {
            for (i, &axis) in axes.iter().enumerate() {
                let color = axis_color(i, hovered, active);
                let tip = center + axis * scale;

                // Shaft
                verts.push(lv(center, color));
                verts.push(lv(tip, color));

                // Small cube at tip
                let cs = scale * 0.05;
                let (p1, p2) = perpendiculars(axis);
                let corners = [
                    tip + p1 * cs + p2 * cs,
                    tip - p1 * cs + p2 * cs,
                    tip - p1 * cs - p2 * cs,
                    tip + p1 * cs - p2 * cs,
                ];
                for j in 0..4 {
                    verts.push(lv(corners[j], color));
                    verts.push(lv(corners[(j + 1) % 4], color));
                }
            }
        }
    }

    verts
}

/// Hit-test the gizmo in screen space. Returns which axis/plane the mouse is over.
pub fn hit_test(
    mouse_pos: Vec2,
    center: Vec3,
    scale: f32,
    mode: GizmoMode,
    view_proj: Mat4,
    screen_size: Vec2,
) -> GizmoAxis {
    let threshold = 12.0; // pixels
    let Some(center_2d) = project_to_screen(center, view_proj, screen_size) else {
        return GizmoAxis::None;
    };

    let axes = [Vec3::X, Vec3::Y, Vec3::Z];
    let axis_ids = [GizmoAxis::X, GizmoAxis::Y, GizmoAxis::Z];

    match mode {
        GizmoMode::Translate | GizmoMode::Scale => {
            // Test plane handles first (they're smaller, should take priority when overlapping)
            if mode == GizmoMode::Translate {
                let psize = scale * 0.12;
                let poff = scale * 0.25;
                let planes = [
                    (0, 1, GizmoAxis::XY),
                    (0, 2, GizmoAxis::XZ),
                    (1, 2, GizmoAxis::YZ),
                ];
                for &(ai, bi, pa) in &planes {
                    let a = axes[ai];
                    let b = axes[bi];
                    let corners = [
                        center + a * poff + b * poff,
                        center + a * (poff + psize) + b * poff,
                        center + a * (poff + psize) + b * (poff + psize),
                        center + a * poff + b * (poff + psize),
                    ];
                    if let (Some(c0), Some(c1), Some(c2), Some(c3)) = (
                        project_to_screen(corners[0], view_proj, screen_size),
                        project_to_screen(corners[1], view_proj, screen_size),
                        project_to_screen(corners[2], view_proj, screen_size),
                        project_to_screen(corners[3], view_proj, screen_size),
                    )
                        && point_in_quad_2d(mouse_pos, c0, c1, c2, c3)
                    {
                        return pa;
                    }
                }
            }

            // Test axis shafts
            let mut best = GizmoAxis::None;
            let mut best_dist = threshold;
            for (i, &axis) in axes.iter().enumerate() {
                let tip = center + axis * scale;
                if let Some(tip_2d) = project_to_screen(tip, view_proj, screen_size) {
                    let d = point_to_segment_dist(mouse_pos, center_2d, tip_2d);
                    if d < best_dist {
                        best_dist = d;
                        best = axis_ids[i];
                    }
                }
            }
            best
        }
        GizmoMode::Rotate => {
            let radius = scale * 0.85;
            let segments = 48;
            let mut best = GizmoAxis::None;
            let mut best_dist = threshold;
            for (i, &axis) in axes.iter().enumerate() {
                let (p1, p2) = perpendiculars(axis);
                for s in 0..segments {
                    let a0 = std::f32::consts::TAU * s as f32 / segments as f32;
                    let a1 = std::f32::consts::TAU * (s + 1) as f32 / segments as f32;
                    let pt0 = center + (p1 * a0.cos() + p2 * a0.sin()) * radius;
                    let pt1 = center + (p1 * a1.cos() + p2 * a1.sin()) * radius;
                    if let (Some(s0), Some(s1)) = (
                        project_to_screen(pt0, view_proj, screen_size),
                        project_to_screen(pt1, view_proj, screen_size),
                    ) {
                        let d = point_to_segment_dist(mouse_pos, s0, s1);
                        if d < best_dist {
                            best_dist = d;
                            best = axis_ids[i];
                        }
                    }
                }
            }
            best
        }
    }
}

/// Project mouse ray onto a constraint axis, returning the world-space point on the axis.
pub fn project_ray_onto_axis(
    ray: &Ray,
    origin: Vec3,
    axis: Vec3,
    camera_forward: Vec3,
) -> Option<Vec3> {
    let plane_normal = constraint_plane_normal(axis, camera_forward);
    let t = ray.intersect_plane(origin, plane_normal)?;
    let point = ray.point_at(t);
    let along = (point - origin).dot(axis);
    Some(origin + axis * along)
}

/// Project mouse ray onto a constraint plane, returning the world-space point.
pub fn project_ray_onto_plane(
    ray: &Ray,
    origin: Vec3,
    plane_normal: Vec3,
) -> Option<Vec3> {
    let t = ray.intersect_plane(origin, plane_normal)?;
    Some(ray.point_at(t))
}

/// Compute the angle from origin in the plane perpendicular to the axis.
pub fn compute_angle_on_axis(point: Vec3, origin: Vec3, axis: Vec3) -> f32 {
    let (p1, p2) = perpendiculars(axis);
    let rel = point - origin;
    let x = rel.dot(p1);
    let y = rel.dot(p2);
    y.atan2(x)
}

/// Get the normal of the best constraint plane containing the axis.
fn constraint_plane_normal(axis: Vec3, camera_forward: Vec3) -> Vec3 {
    let cross = camera_forward.cross(axis);
    if cross.length_squared() < 1e-6 {
        // Camera looking down the axis — pick any perpendicular
        let fallback = if axis.x.abs() < 0.9 { Vec3::X } else { Vec3::Y };
        fallback.cross(axis).normalize()
    } else {
        cross.cross(axis).normalize()
    }
}

/// Get the plane normal for a GizmoAxis plane handle.
pub fn plane_normal_for_axis(axis: GizmoAxis) -> Vec3 {
    match axis {
        GizmoAxis::XY => Vec3::Z,
        GizmoAxis::XZ => Vec3::Y,
        GizmoAxis::YZ => Vec3::X,
        _ => Vec3::Y,
    }
}

// --- helpers ---

fn axis_color(idx: usize, hovered: GizmoAxis, active: GizmoAxis) -> [f32; 4] {
    let ga = match idx {
        0 => GizmoAxis::X,
        1 => GizmoAxis::Y,
        _ => GizmoAxis::Z,
    };
    if active == ga || hovered == ga {
        HIGHLIGHT_COLOR
    } else {
        AXIS_COLORS[idx]
    }
}

fn plane_color(plane: GizmoAxis, hovered: GizmoAxis, active: GizmoAxis) -> [f32; 4] {
    if active == plane || hovered == plane {
        [1.0, 1.0, 0.3, 0.8]
    } else {
        match plane {
            GizmoAxis::XY => [0.4, 0.4, 1.0, 0.5],
            GizmoAxis::XZ => [0.4, 1.0, 0.4, 0.5],
            GizmoAxis::YZ => [1.0, 0.4, 0.4, 0.5],
            _ => [0.5, 0.5, 0.5, 0.5],
        }
    }
}

fn perpendiculars(axis: Vec3) -> (Vec3, Vec3) {
    let ref_vec = if axis.y.abs() < 0.9 { Vec3::Y } else { Vec3::X };
    let p1 = axis.cross(ref_vec).normalize();
    let p2 = axis.cross(p1).normalize();
    (p1, p2)
}

fn lv(pos: Vec3, color: [f32; 4]) -> LineVertex {
    LineVertex {
        position: pos.into(),
        color,
    }
}

/// Distance from a point to a line segment in 2D.
fn point_to_segment_dist(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let len_sq = ab.length_squared();
    if len_sq < 1e-6 {
        return p.distance(a);
    }
    let t = ((p - a).dot(ab) / len_sq).clamp(0.0, 1.0);
    let proj = a + ab * t;
    p.distance(proj)
}

/// Test if a 2D point is inside a convex quad (4 vertices in order).
fn point_in_quad_2d(p: Vec2, a: Vec2, b: Vec2, c: Vec2, d: Vec2) -> bool {
    point_in_tri_2d(p, a, b, c) || point_in_tri_2d(p, a, c, d)
}

fn point_in_tri_2d(p: Vec2, a: Vec2, b: Vec2, c: Vec2) -> bool {
    let v0 = c - a;
    let v1 = b - a;
    let v2 = p - a;
    let d00 = v0.dot(v0);
    let d01 = v0.dot(v1);
    let d02 = v0.dot(v2);
    let d11 = v1.dot(v1);
    let d12 = v1.dot(v2);
    let inv = 1.0 / (d00 * d11 - d01 * d01);
    let u = (d11 * d02 - d01 * d12) * inv;
    let v = (d00 * d12 - d01 * d02) * inv;
    u >= 0.0 && v >= 0.0 && u + v <= 1.0
}
