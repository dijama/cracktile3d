use glam::{Vec2, Vec3, Mat4};
use crate::util::picking::project_to_screen;

const RULER_SIZE: f32 = 20.0;
const TICK_COLOR: egui::Color32 = egui::Color32::from_rgb(160, 160, 160);
const LABEL_COLOR: egui::Color32 = egui::Color32::from_rgb(180, 180, 180);
const BG_COLOR: egui::Color32 = egui::Color32::from_rgba_premultiplied(30, 30, 35, 220);

/// Draw rulers along the top and left edges of the viewport.
pub fn draw_rulers(
    ctx: &egui::Context,
    view_proj: Mat4,
    screen_size: Vec2,
    grid_size: f32,
    crosshair_y: f32,
) {
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("rulers"),
    ));

    let screen_w = screen_size.x;
    let screen_h = screen_size.y;

    // Background strips
    painter.rect_filled(
        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(screen_w, RULER_SIZE)),
        0.0, BG_COLOR,
    );
    painter.rect_filled(
        egui::Rect::from_min_max(egui::pos2(0.0, RULER_SIZE), egui::pos2(RULER_SIZE, screen_h)),
        0.0, BG_COLOR,
    );

    // Determine visible world range by unprojecting screen corners
    // Instead of unprojecting (complex), project a range of world positions and draw those that land on screen
    let range = compute_visible_range(view_proj, screen_size, grid_size);

    // Horizontal ruler: project X positions along the crosshair plane (Z=0)
    draw_horizontal_ruler(&painter, view_proj, screen_size, grid_size, crosshair_y, range);

    // Vertical ruler: project Y positions along the crosshair plane (X=0)
    draw_vertical_ruler(&painter, view_proj, screen_size, grid_size, range);
}

/// Estimate the visible world range by checking how many grid units fit on screen.
fn compute_visible_range(_view_proj: Mat4, _screen_size: Vec2, grid_size: f32) -> f32 {
    // Return a generous range; we'll cull ticks that are off-screen
    // A typical view distance: 50 world units should cover most zoom levels
    let max_units = 200.0;
    // Round up to grid step
    (max_units / grid_size).ceil() * grid_size
}

fn draw_horizontal_ruler(
    painter: &egui::Painter,
    view_proj: Mat4,
    screen_size: Vec2,
    grid_size: f32,
    crosshair_y: f32,
    range: f32,
) {
    // Choose tick step: increase step if ticks would be too dense
    let step = adaptive_step(view_proj, screen_size, grid_size, Vec3::X);

    let mut x = -range;
    while x <= range {
        let world_pos = Vec3::new(x, crosshair_y, 0.0);
        if let Some(sp) = project_to_screen(world_pos, view_proj, screen_size)
            && sp.x >= RULER_SIZE && sp.x <= screen_size.x
        {
            let sx = sp.x;
            // Major tick
            painter.line_segment(
                [egui::pos2(sx, 0.0), egui::pos2(sx, RULER_SIZE)],
                egui::Stroke::new(1.0, TICK_COLOR),
            );
            // Label
            let label = format_coord(x);
            painter.text(
                egui::pos2(sx + 2.0, 2.0),
                egui::Align2::LEFT_TOP,
                &label,
                egui::FontId::monospace(9.0),
                LABEL_COLOR,
            );
        }
        x += step;
    }
}

fn draw_vertical_ruler(
    painter: &egui::Painter,
    view_proj: Mat4,
    screen_size: Vec2,
    grid_size: f32,
    range: f32,
) {
    // Vertical ruler shows Y coordinates (height)
    let step = adaptive_step(view_proj, screen_size, grid_size, Vec3::Y);

    let mut y = -range;
    while y <= range {
        let world_pos = Vec3::new(0.0, y, 0.0);
        if let Some(sp) = project_to_screen(world_pos, view_proj, screen_size)
            && sp.y >= RULER_SIZE && sp.y <= screen_size.y
        {
            let sy = sp.y;
            // Major tick
            painter.line_segment(
                [egui::pos2(0.0, sy), egui::pos2(RULER_SIZE, sy)],
                egui::Stroke::new(1.0, TICK_COLOR),
            );
            // Label
            let label = format_coord(y);
            painter.text(
                egui::pos2(2.0, sy - 10.0),
                egui::Align2::LEFT_TOP,
                &label,
                egui::FontId::monospace(9.0),
                LABEL_COLOR,
            );
        }
        y += step;
    }
}

/// Choose a tick step that ensures labels don't overlap.
/// Projects two adjacent grid points and checks pixel distance.
fn adaptive_step(view_proj: Mat4, screen_size: Vec2, grid_size: f32, axis: Vec3) -> f32 {
    let mut step = grid_size;
    // Project origin and origin+step to see pixel distance
    let p0 = project_to_screen(Vec3::ZERO, view_proj, screen_size);
    let p1 = project_to_screen(axis * step, view_proj, screen_size);
    if let (Some(a), Some(b)) = (p0, p1) {
        let pixel_dist = if axis.x > 0.5 { (b.x - a.x).abs() } else { (b.y - a.y).abs() };
        // If ticks are too dense (< 30 pixels apart), increase step
        if pixel_dist > 0.1 {
            let min_spacing = 40.0;
            while {
                let p_test = project_to_screen(axis * step, view_proj, screen_size);
                if let (Some(a2), Some(b2)) = (p0, p_test) {
                    let d = if axis.x > 0.5 { (b2.x - a2.x).abs() } else { (b2.y - a2.y).abs() };
                    d < min_spacing
                } else {
                    false
                }
            } {
                step *= 2.0;
                if step > 1000.0 { break; }
            }
        }
    }
    step
}

/// Format a coordinate value for display.
fn format_coord(v: f32) -> String {
    if v == 0.0 {
        "0".to_string()
    } else if v.fract().abs() < 0.001 {
        format!("{}", v as i32)
    } else {
        format!("{:.1}", v)
    }
}
