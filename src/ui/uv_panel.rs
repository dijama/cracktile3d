use glam::Vec2;
use crate::scene::Scene;
use crate::tools::edit::EditState;
use super::UiAction;

/// State for the UV editing panel.
pub struct UvPanelState {
    /// Whether the UV panel window is open.
    pub open: bool,
    /// Selected UV vertex indices: (index into selection.faces, vertex 0-3)
    pub selected_uv_verts: Vec<(usize, usize)>,
    /// Active UV drag state
    uv_drag: Option<UvDrag>,
    /// Zoom level for the UV viewport
    pub zoom: f32,
}

struct UvDrag {
    /// Targets: (index into selection.faces, vertex 0-3)
    targets: Vec<(usize, usize)>,
    /// Mouse position at drag start (in UV space)
    start_uv: Vec2,
    /// Original UV positions of targets before drag
    original_uvs: Vec<Vec2>,
}

impl UvPanelState {
    pub fn new() -> Self {
        Self {
            open: false,
            selected_uv_verts: Vec::new(),
            uv_drag: None,
            zoom: 1.0,
        }
    }
}

/// Draw the UV editing panel as a floating window.
pub fn draw_uv_panel(
    ctx: &egui::Context,
    scene: &mut Scene,
    edit_state: &EditState,
    uv_state: &mut UvPanelState,
) -> UiAction {
    if !uv_state.open {
        return UiAction::None;
    }

    let mut action = UiAction::None;
    let mut open = true;

    egui::Window::new("UV Editor")
        .id(egui::Id::new("uv_editor_panel"))
        .open(&mut open)
        .resizable(true)
        .default_size([400.0, 450.0])
        .show(ctx, |ui| {
            action = draw_uv_content(ui, scene, edit_state, uv_state);
        });

    if !open {
        uv_state.open = false;
    }

    action
}

fn draw_uv_content(
    ui: &mut egui::Ui,
    scene: &mut Scene,
    edit_state: &EditState,
    uv_state: &mut UvPanelState,
) -> UiAction {
    let mut action = UiAction::None;

    // UV operation buttons
    ui.horizontal(|ui| {
        ui.label("UV Ops:");
        if ui.small_button("Rot CW").on_hover_text("Rotate UVs clockwise").clicked() {
            action = UiAction::UVRotateCW;
        }
        if ui.small_button("Rot CCW").on_hover_text("Rotate UVs counter-clockwise").clicked() {
            action = UiAction::UVRotateCCW;
        }
        if ui.small_button("Flip H").on_hover_text("Flip UVs horizontally").clicked() {
            action = UiAction::UVFlipH;
        }
        if ui.small_button("Flip V").on_hover_text("Flip UVs vertically").clicked() {
            action = UiAction::UVFlipV;
        }
    });

    // Zoom controls
    ui.horizontal(|ui| {
        ui.label(format!("Zoom: {:.0}%", uv_state.zoom * 100.0));
        if ui.small_button("-").clicked() {
            uv_state.zoom = (uv_state.zoom - 0.25).max(0.5);
        }
        if ui.small_button("+").clicked() {
            uv_state.zoom = (uv_state.zoom + 0.25).min(4.0);
        }
        if ui.small_button("Fit").clicked() {
            uv_state.zoom = 1.0;
        }
    });

    ui.separator();

    if edit_state.selection.faces.is_empty() {
        ui.label("Select faces in Edit mode to view UVs.");
        return action;
    }

    // Get active tileset info
    let tileset_info = scene.active_tileset.and_then(|idx| {
        scene.tilesets.get(idx).and_then(|ts| {
            ts.egui_texture_id.map(|tex_id| (tex_id, ts.image_width, ts.image_height))
        })
    });

    let Some((tex_id, img_w, img_h)) = tileset_info else {
        ui.label("No tileset loaded. Load a tileset to see UV mapping.");
        draw_uv_text_fallback(ui, scene, edit_state);
        return action;
    };

    // UV viewport: tileset image with UV overlays
    let available = ui.available_size();
    let img_aspect = img_w as f32 / img_h as f32;
    let base_w = available.x.max(100.0);
    let base_h = base_w / img_aspect;
    let display_w = base_w * uv_state.zoom;
    let display_h = base_h * uv_state.zoom;

    egui::ScrollArea::both().show(ui, |ui| {
        let (response, painter) = ui.allocate_painter(
            egui::vec2(display_w, display_h),
            egui::Sense::click_and_drag(),
        );
        let rect = response.rect;

        // Handle scroll-to-zoom
        let hover_pos = ui.input(|i| i.pointer.hover_pos());
        if let Some(hp) = hover_pos
            && rect.contains(hp)
        {
            let scroll = ui.input(|i| i.raw_scroll_delta.y);
            if scroll != 0.0 {
                let delta = if scroll > 0.0 { 0.25 } else { -0.25 };
                uv_state.zoom = (uv_state.zoom + delta).clamp(0.5, 4.0);
            }
        }

        // Draw the tileset image as background
        painter.image(
            tex_id,
            rect,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            egui::Color32::from_rgba_unmultiplied(255, 255, 255, 180),
        );

        // Draw 0..1 UV space border
        painter.rect_stroke(
            rect,
            0.0,
            egui::Stroke::new(1.0, egui::Color32::from_gray(80)),
            egui::StrokeKind::Outside,
        );

        // Helper: map UV (0..1) to screen position
        let uv_to_screen = |uv: Vec2| -> egui::Pos2 {
            egui::pos2(
                rect.left() + uv.x * rect.width(),
                rect.top() + uv.y * rect.height(),
            )
        };

        // Helper: map screen position to UV (0..1)
        let screen_to_uv = |pos: egui::Pos2| -> Vec2 {
            Vec2::new(
                (pos.x - rect.left()) / rect.width(),
                (pos.y - rect.top()) / rect.height(),
            )
        };

        // Draw UV wireframes for selected faces
        let face_color = egui::Color32::from_rgb(100, 200, 255);
        let selected_vert_color = egui::Color32::YELLOW;
        let vert_color = egui::Color32::WHITE;
        let vert_radius = 4.0;

        for (sel_idx, &(li, oi, fi)) in edit_state.selection.faces.iter().enumerate() {
            let Some(face) = scene.layers.get(li)
                .and_then(|l| l.objects.get(oi))
                .and_then(|o| o.faces.get(fi))
            else {
                continue;
            };

            // Draw quad edges
            for i in 0..4 {
                let a = uv_to_screen(face.uvs[i]);
                let b = uv_to_screen(face.uvs[(i + 1) % 4]);
                painter.line_segment([a, b], egui::Stroke::new(1.5, face_color));
            }

            // Draw vertex handles
            for vi in 0..4 {
                let pos = uv_to_screen(face.uvs[vi]);
                let is_selected = uv_state.selected_uv_verts.contains(&(sel_idx, vi));
                let color = if is_selected { selected_vert_color } else { vert_color };
                painter.circle_filled(pos, vert_radius, color);
                painter.circle_stroke(pos, vert_radius, egui::Stroke::new(1.0, egui::Color32::BLACK));
            }
        }

        // Handle UV vertex selection on click
        if response.clicked()
            && let Some(pos) = response.interact_pointer_pos()
        {
            let click_uv = screen_to_uv(pos);
            let threshold = 8.0 / rect.width().max(1.0);

            let shift = ui.input(|i| i.modifiers.shift);
            if !shift {
                uv_state.selected_uv_verts.clear();
            }

            let mut best_dist = threshold;
            let mut best_entry = None;

            for (sel_idx, &(li, oi, fi)) in edit_state.selection.faces.iter().enumerate() {
                if let Some(face) = scene.layers.get(li)
                    .and_then(|l| l.objects.get(oi))
                    .and_then(|o| o.faces.get(fi))
                {
                    for vi in 0..4 {
                        let dist = (face.uvs[vi] - click_uv).length();
                        if dist < best_dist {
                            best_dist = dist;
                            best_entry = Some((sel_idx, vi));
                        }
                    }
                }
            }

            if let Some(entry) = best_entry
                && !uv_state.selected_uv_verts.contains(&entry)
            {
                uv_state.selected_uv_verts.push(entry);
            }
        }

        // Handle UV vertex dragging — start
        if response.drag_started() && !uv_state.selected_uv_verts.is_empty()
            && let Some(pos) = response.interact_pointer_pos()
        {
            let start_uv = screen_to_uv(pos);

            // Check if drag started near a selected UV vertex
            let threshold = 10.0 / rect.width().max(1.0);
            let mut near_selected = false;
            for &(sel_idx, vi) in &uv_state.selected_uv_verts {
                if let Some(&(li, oi, fi)) = edit_state.selection.faces.get(sel_idx)
                    && let Some(face) = scene.layers.get(li)
                        .and_then(|l| l.objects.get(oi))
                        .and_then(|o| o.faces.get(fi))
                    && (face.uvs[vi] - start_uv).length() < threshold
                {
                    near_selected = true;
                    break;
                }
            }

            if near_selected {
                let mut original_uvs = Vec::new();
                for &(sel_idx, vi) in &uv_state.selected_uv_verts {
                    if let Some(&(li, oi, fi)) = edit_state.selection.faces.get(sel_idx)
                        && let Some(face) = scene.layers.get(li)
                            .and_then(|l| l.objects.get(oi))
                            .and_then(|o| o.faces.get(fi))
                    {
                        original_uvs.push(face.uvs[vi]);
                    } else {
                        original_uvs.push(Vec2::ZERO);
                    }
                }

                uv_state.uv_drag = Some(UvDrag {
                    targets: uv_state.selected_uv_verts.clone(),
                    start_uv,
                    original_uvs,
                });
            }
        }

        // Handle UV vertex dragging — update
        if response.dragged()
            && let Some(ref drag) = uv_state.uv_drag
            && let Some(pos) = response.interact_pointer_pos()
        {
            let current_uv = screen_to_uv(pos);
            let delta = current_uv - drag.start_uv;

            for (i, &(sel_idx, vi)) in drag.targets.iter().enumerate() {
                if let Some(&(li, oi, fi)) = edit_state.selection.faces.get(sel_idx)
                    && let Some(face) = scene.layers.get_mut(li)
                        .and_then(|l| l.objects.get_mut(oi))
                        .and_then(|o| o.faces.get_mut(fi))
                {
                    face.uvs[vi] = drag.original_uvs[i] + delta;
                }
            }

            // Mark objects as dirty for GPU mesh rebuild
            let mut dirty: std::collections::HashSet<(usize, usize)> = std::collections::HashSet::new();
            for &(sel_idx, _) in &drag.targets {
                if let Some(&(li, oi, _)) = edit_state.selection.faces.get(sel_idx) {
                    dirty.insert((li, oi));
                }
            }
            for (li, oi) in dirty {
                scene.dirty_objects.push((li, oi));
            }
        }

        // Handle UV vertex dragging — commit
        if response.drag_stopped()
            && let Some(drag) = uv_state.uv_drag.take()
        {
            // Restore original UVs so the undo command can apply cleanly
            for (i, &(sel_idx, vi)) in drag.targets.iter().enumerate() {
                if let Some(&(li, oi, fi)) = edit_state.selection.faces.get(sel_idx)
                    && let Some(face) = scene.layers.get_mut(li)
                        .and_then(|l| l.objects.get_mut(oi))
                        .and_then(|o| o.faces.get_mut(fi))
                {
                    face.uvs[vi] = drag.original_uvs[i];
                }
            }

            // Compute final delta and emit action
            if let Some(pos) = ui.input(|i| i.pointer.latest_pos()) {
                let final_uv = screen_to_uv(pos);
                let delta = final_uv - drag.start_uv;
                if delta.length_squared() > 1e-8 {
                    action = UiAction::UvVertexDrag {
                        targets: drag.targets,
                        original_uvs: drag.original_uvs,
                        delta,
                    };
                }
            }

            // Mark dirty for rebuild
            let mut dirty: std::collections::HashSet<(usize, usize)> = std::collections::HashSet::new();
            for &(sel_idx, _) in &uv_state.selected_uv_verts {
                if let Some(&(li, oi, _)) = edit_state.selection.faces.get(sel_idx) {
                    dirty.insert((li, oi));
                }
            }
            for (li, oi) in dirty {
                scene.dirty_objects.push((li, oi));
            }
        }
    });

    action
}

/// Fallback: show UV coordinates as text when no tileset is available.
fn draw_uv_text_fallback(
    ui: &mut egui::Ui,
    scene: &Scene,
    edit_state: &EditState,
) {
    for &(li, oi, fi) in &edit_state.selection.faces {
        if let Some(face) = scene.layers.get(li)
            .and_then(|l| l.objects.get(oi))
            .and_then(|o| o.faces.get(fi))
        {
            ui.label(format!("Face [{li},{oi},{fi}]:"));
            for (vi, uv) in face.uvs.iter().enumerate() {
                ui.label(format!("  v{vi}: ({:.4}, {:.4})", uv.x, uv.y));
            }
        }
    }
}
