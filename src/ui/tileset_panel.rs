use crate::scene::Scene;
use crate::tools::draw::DrawState;

/// Actions the tileset panel wants the app to execute.
pub enum TilesetAction {
    None,
    LoadTileset,
}

/// Draw the tileset browser panel (bottom).
pub fn draw_tileset_panel(
    ctx: &egui::Context,
    scene: &mut Scene,
    draw_state: &mut DrawState,
) -> TilesetAction {
    let mut action = TilesetAction::None;

    egui::TopBottomPanel::bottom("tileset_panel")
        .default_height(280.0)
        .resizable(true)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Tileset");

                // Tileset selector dropdown
                if !scene.tilesets.is_empty() {
                    let current_name = scene.active_tileset
                        .and_then(|i| scene.tilesets.get(i))
                        .map(|t| t.name.clone())
                        .unwrap_or_else(|| "None".to_string());

                    egui::ComboBox::from_id_salt("tileset_selector")
                        .selected_text(&current_name)
                        .show_ui(ui, |ui| {
                            for (i, tileset) in scene.tilesets.iter().enumerate() {
                                let selected = scene.active_tileset == Some(i);
                                if ui.selectable_label(selected, &tileset.name).clicked() {
                                    scene.active_tileset = Some(i);
                                }
                            }
                        });
                }

                if ui.button("Load...").clicked() {
                    action = TilesetAction::LoadTileset;
                }

                // Zoom controls
                ui.separator();
                ui.label(format!("{:.0}%", draw_state.tileset_zoom * 100.0));
                if ui.small_button("-").clicked() {
                    draw_state.tileset_zoom = (draw_state.tileset_zoom - 0.25).max(0.25);
                }
                if ui.small_button("+").clicked() {
                    draw_state.tileset_zoom = (draw_state.tileset_zoom + 0.25).min(8.0);
                }
                if ui.small_button("Fit").clicked() {
                    draw_state.tileset_zoom = 1.0;
                }
            });

            ui.separator();

            // Show the active tileset image with clickable grid
            if let Some(active_idx) = scene.active_tileset {
                if let Some(tileset) = scene.tilesets.get(active_idx) {
                    if let Some(tex_id) = tileset.egui_texture_id {
                        let cols = tileset.cols();
                        let rows = tileset.rows();

                        if cols == 0 || rows == 0 {
                            ui.label("Tileset has no tiles (check tile size).");
                            return;
                        }

                        // Scale tileset to fit available width, then apply zoom multiplier.
                        // This ensures tiles are large enough to see and click regardless
                        // of the native tileset pixel size.
                        let available_width = ui.available_width().max(100.0);
                        let img_aspect = tileset.image_width as f32 / tileset.image_height as f32;

                        // Base size: fit tileset width to panel width
                        let base_width = available_width;
                        let base_height = base_width / img_aspect;

                        // Apply zoom
                        let display_width = base_width * draw_state.tileset_zoom;
                        let display_height = base_height * draw_state.tileset_zoom;
                        let display_size = egui::vec2(display_width, display_height);

                        // Wrap in scroll area for zoom
                        egui::ScrollArea::both().show(ui, |ui| {
                            let (response, painter) = ui.allocate_painter(display_size, egui::Sense::click_and_drag());
                            let rect = response.rect;

                            // Handle scroll-to-zoom on the tileset area
                            let hover_pos = ui.input(|i| i.pointer.hover_pos());
                            if let Some(hp) = hover_pos
                                && rect.contains(hp)
                            {
                                let scroll = ui.input(|i| i.raw_scroll_delta.y);
                                if scroll != 0.0 {
                                    let delta = if scroll > 0.0 { 0.25 } else { -0.25 };
                                    draw_state.tileset_zoom = (draw_state.tileset_zoom + delta).clamp(0.25, 8.0);
                                }
                            }

                            // Draw the tileset image
                            painter.image(
                                tex_id,
                                rect,
                                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                                egui::Color32::WHITE,
                            );

                            // Draw grid lines
                            let cell_w = rect.width() / cols as f32;
                            let cell_h = rect.height() / rows as f32;
                            let grid_color = egui::Color32::from_rgba_premultiplied(100, 100, 100, 120);

                            for c in 0..=cols {
                                let x = rect.left() + c as f32 * cell_w;
                                painter.line_segment(
                                    [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
                                    egui::Stroke::new(1.0, grid_color),
                                );
                            }
                            for r in 0..=rows {
                                let y = rect.top() + r as f32 * cell_h;
                                painter.line_segment(
                                    [egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)],
                                    egui::Stroke::new(1.0, grid_color),
                                );
                            }

                            // Highlight selected tile region with filled overlay + thick border
                            let c0 = draw_state.selected_tile.0.min(draw_state.selected_tile_end.0);
                            let c1 = draw_state.selected_tile.0.max(draw_state.selected_tile_end.0);
                            let r0 = draw_state.selected_tile.1.min(draw_state.selected_tile_end.1);
                            let r1 = draw_state.selected_tile.1.max(draw_state.selected_tile_end.1);

                            if c0 < cols && r0 < rows {
                                let sel_rect = egui::Rect::from_min_max(
                                    egui::pos2(
                                        rect.left() + c0 as f32 * cell_w,
                                        rect.top() + r0 as f32 * cell_h,
                                    ),
                                    egui::pos2(
                                        rect.left() + (c1 + 1).min(cols) as f32 * cell_w,
                                        rect.top() + (r1 + 1).min(rows) as f32 * cell_h,
                                    ),
                                );
                                // Semi-transparent yellow fill so selection is visible even on tiny tiles
                                painter.rect_filled(
                                    sel_rect,
                                    0.0,
                                    egui::Color32::from_rgba_unmultiplied(255, 255, 0, 60),
                                );
                                painter.rect_stroke(
                                    sel_rect,
                                    0.0,
                                    egui::Stroke::new(2.0, egui::Color32::YELLOW),
                                    egui::StrokeKind::Outside,
                                );
                            }

                            // Handle click/drag to select tile(s)
                            if response.drag_started()
                                && let Some(pos) = response.interact_pointer_pos()
                            {
                                let local = pos - rect.left_top();
                                let col = (local.x / cell_w) as u32;
                                let row = (local.y / cell_h) as u32;
                                if col < cols && row < rows {
                                    draw_state.selected_tile = (col, row);
                                    draw_state.selected_tile_end = (col, row);
                                }
                            }

                            if response.dragged()
                                && let Some(pos) = response.interact_pointer_pos()
                            {
                                let local = pos - rect.left_top();
                                let col = ((local.x / cell_w) as u32).min(cols.saturating_sub(1));
                                let row = ((local.y / cell_h) as u32).min(rows.saturating_sub(1));
                                draw_state.selected_tile_end = (col, row);
                            }
                        });
                    } else {
                        ui.label("Tileset texture not registered with UI");
                    }
                }
            } else if scene.tilesets.is_empty() {
                ui.label("No tileset loaded. Click 'Load...' to add one.");
            }
        });

    action
}
