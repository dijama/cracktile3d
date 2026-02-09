use glam::{Mat4, Vec3};

/// Which part of the ViewCube was clicked.
pub enum ViewCubeClick {
    Front,
    Back,
    Left,
    Right,
    Top,
    Bottom,
}

/// Draw the ViewCube in the top-right corner of the screen.
/// Returns a `ViewCubeClick` if the user clicked on a face.
pub fn draw_viewcube(ctx: &egui::Context, yaw: f32, pitch: f32) -> Option<ViewCubeClick> {
    let mut clicked = None;

    // ViewCube parameters
    let cube_size = 50.0; // half-extent in screen pixels
    let margin = 16.0;

    let screen_rect = ctx.screen_rect();
    let center_x = screen_rect.max.x - cube_size - margin;
    let center_y = cube_size + margin;
    let center = egui::pos2(center_x, center_y);

    // Build rotation matrix from camera yaw/pitch (camera looks at -Z by default)
    let rot = Mat4::from_rotation_x(-pitch) * Mat4::from_rotation_y(-yaw);

    // Unit cube vertices (±1)
    let verts: [Vec3; 8] = [
        Vec3::new(-1.0, -1.0, -1.0), // 0: left-bottom-front
        Vec3::new( 1.0, -1.0, -1.0), // 1: right-bottom-front
        Vec3::new( 1.0,  1.0, -1.0), // 2: right-top-front
        Vec3::new(-1.0,  1.0, -1.0), // 3: left-top-front
        Vec3::new(-1.0, -1.0,  1.0), // 4: left-bottom-back
        Vec3::new( 1.0, -1.0,  1.0), // 5: right-bottom-back
        Vec3::new( 1.0,  1.0,  1.0), // 6: right-top-back
        Vec3::new(-1.0,  1.0,  1.0), // 7: left-top-back
    ];

    // Project each vertex to 2D (simple orthographic projection of rotated vertices)
    let project = |v: Vec3| -> egui::Pos2 {
        let rotated = rot.transform_point3(v);
        // X goes right, Y goes up (negate for screen coords where Y goes down)
        egui::pos2(
            center.x + rotated.x * cube_size * 0.45,
            center.y - rotated.y * cube_size * 0.45,
        )
    };

    let projected: Vec<egui::Pos2> = verts.iter().map(|&v| project(v)).collect();

    // Face definitions: (vertex indices, label, normal, click action)
    let faces: [([ usize; 4], &str, Vec3, ViewCubeClick); 6] = [
        ([0, 1, 2, 3], "Front",  Vec3::new(0.0, 0.0, -1.0), ViewCubeClick::Front),
        ([5, 4, 7, 6], "Back",   Vec3::new(0.0, 0.0,  1.0), ViewCubeClick::Back),
        ([4, 0, 3, 7], "Left",   Vec3::new(-1.0, 0.0, 0.0), ViewCubeClick::Left),
        ([1, 5, 6, 2], "Right",  Vec3::new(1.0, 0.0, 0.0),  ViewCubeClick::Right),
        ([3, 2, 6, 7], "Top",    Vec3::new(0.0, 1.0, 0.0),  ViewCubeClick::Top),
        ([0, 4, 5, 1], "Bottom", Vec3::new(0.0, -1.0, 0.0), ViewCubeClick::Bottom),
    ];

    // Sort faces back-to-front by average rotated Z
    let mut face_order: Vec<(usize, f32)> = faces.iter().enumerate().map(|(i, (_, _, normal, _))| {
        let rotated_normal = rot.transform_vector3(*normal);
        (i, rotated_normal.z) // more negative Z = facing camera
    }).collect();
    face_order.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("viewcube"),
    ));

    // Check hover
    let mouse_pos = ctx.input(|i| i.pointer.hover_pos());
    let mouse_clicked = ctx.input(|i| i.pointer.primary_clicked());

    // Draw faces back-to-front
    for &(fi, _z_depth) in &face_order {
        let (ref indices, label, _normal, _) = faces[fi];
        let rotated_normal = rot.transform_vector3(faces[fi].2);

        // Only draw faces facing the camera (normal.z < 0 means facing towards us)
        if rotated_normal.z > 0.1 {
            continue; // back-facing
        }

        let pts: Vec<egui::Pos2> = indices.iter().map(|&i| projected[i]).collect();

        // Compute face bounding polygon for hover detection
        let hovering = mouse_pos.is_some_and(|mp| point_in_quad(mp, &pts));

        // Face fill color
        let alpha = ((-rotated_normal.z).clamp(0.0, 1.0) * 200.0) as u8 + 40;
        let fill = if hovering {
            egui::Color32::from_rgba_unmultiplied(100, 160, 255, alpha)
        } else {
            egui::Color32::from_rgba_unmultiplied(60, 60, 80, alpha)
        };

        // Draw filled quad as two triangles
        let mesh = egui::Mesh {
            indices: vec![0, 1, 2, 0, 2, 3],
            vertices: pts.iter().map(|&p| egui::epaint::Vertex {
                pos: p,
                uv: egui::pos2(0.0, 0.0),
                color: fill,
            }).collect(),
            texture_id: egui::TextureId::default(),
        };
        painter.add(egui::Shape::mesh(mesh));

        // Draw edges
        let edge_color = egui::Color32::from_rgba_unmultiplied(200, 200, 220, 180);
        for i in 0..4 {
            let a = pts[i];
            let b = pts[(i + 1) % 4];
            painter.line_segment([a, b], egui::Stroke::new(1.0, edge_color));
        }

        // Draw label (centered on face)
        let face_center = egui::pos2(
            pts.iter().map(|p| p.x).sum::<f32>() / 4.0,
            pts.iter().map(|p| p.y).sum::<f32>() / 4.0,
        );
        let text_color = if hovering {
            egui::Color32::WHITE
        } else {
            egui::Color32::from_rgba_unmultiplied(220, 220, 220, alpha)
        };
        painter.text(
            face_center,
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::proportional(10.0),
            text_color,
        );

        // Handle click
        if hovering && mouse_clicked {
            clicked = Some(match fi {
                0 => ViewCubeClick::Front,
                1 => ViewCubeClick::Back,
                2 => ViewCubeClick::Left,
                3 => ViewCubeClick::Right,
                4 => ViewCubeClick::Top,
                5 => ViewCubeClick::Bottom,
                _ => unreachable!(),
            });
        }
    }

    clicked
}

/// Point-in-quad test using cross products (convex polygon).
fn point_in_quad(p: egui::Pos2, quad: &[egui::Pos2]) -> bool {
    if quad.len() < 3 { return false; }
    let n = quad.len();
    let mut sign = 0i32;
    for i in 0..n {
        let a = quad[i];
        let b = quad[(i + 1) % n];
        let cross = (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x);
        if cross.abs() < 1e-6 { continue; }
        let s = if cross > 0.0 { 1 } else { -1 };
        if sign == 0 {
            sign = s;
        } else if sign != s {
            return false;
        }
    }
    true
}
