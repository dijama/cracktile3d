use crate::scene::Scene;
use crate::tile::{FilterMode, WrapMode, AlphaMode};
use crate::tile::palette::{PaletteMode};
use crate::tools::draw::DrawState;

/// Actions the tileset panel wants the app to execute.
pub enum TilesetAction {
    None,
    LoadTileset,
    RemoveTileset(usize),
    DuplicateTileset(usize),
    ReplaceTileset(usize),
    ExportTileset(usize),
    RemoveUnusedTilesets,
    OpenPaintEditor,
    /// Material settings changed — rebuild sampler/bind_group for this tileset.
    RebuildMaterial(usize),
}

/// Draw the tileset browser panel — dispatches to docked or floating mode.
pub fn draw_tileset_panel(
    ctx: &egui::Context,
    scene: &mut Scene,
    draw_state: &mut DrawState,
) -> TilesetAction {
    if draw_state.tileset_panel_floating {
        draw_tileset_panel_floating(ctx, scene, draw_state)
    } else {
        draw_tileset_panel_docked(ctx, scene, draw_state)
    }
}

/// Docked mode: tileset panel as a bottom panel (original behavior).
fn draw_tileset_panel_docked(
    ctx: &egui::Context,
    scene: &mut Scene,
    draw_state: &mut DrawState,
) -> TilesetAction {
    let mut action = TilesetAction::None;

    egui::TopBottomPanel::bottom("tileset_panel")
        .default_height(280.0)
        .resizable(true)
        .show(ctx, |ui| {
            action = draw_tileset_content(ui, scene, draw_state);
        });

    action
}

/// Floating mode: tileset panel as a movable, resizable egui::Window.
fn draw_tileset_panel_floating(
    ctx: &egui::Context,
    scene: &mut Scene,
    draw_state: &mut DrawState,
) -> TilesetAction {
    let mut action = TilesetAction::None;
    let mut open = true;

    egui::Window::new("Tileset")
        .id(egui::Id::new("tileset_panel_floating"))
        .open(&mut open)
        .resizable(true)
        .collapsible(true)
        .default_size([400.0, 350.0])
        .show(ctx, |ui| {
            action = draw_tileset_content(ui, scene, draw_state);
        });

    // If the user closed the floating window via X, revert to docked
    if !open {
        draw_state.tileset_panel_floating = false;
    }

    action
}

/// Shared tileset panel content rendered inside either container.
fn draw_tileset_content(
    ui: &mut egui::Ui,
    scene: &mut Scene,
    draw_state: &mut DrawState,
) -> TilesetAction {
    let mut action = TilesetAction::None;

    ui.horizontal(|ui| {
        ui.heading("Tileset");

        // Pop-out / dock toggle button
        let icon = if draw_state.tileset_panel_floating { "\u{1f4cc}" } else { "\u{1f5d7}" };
        let tooltip = if draw_state.tileset_panel_floating { "Dock panel" } else { "Float panel" };
        if ui.small_button(icon).on_hover_text(tooltip).clicked() {
            draw_state.tileset_panel_floating = !draw_state.tileset_panel_floating;
        }

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

        // Tileset management context menu
        if let Some(idx) = scene.active_tileset {
            ui.menu_button("Manage", |ui| {
                if ui.button("Replace Image...").on_hover_text("Replace tileset image, keeping UV references").clicked() {
                    action = TilesetAction::ReplaceTileset(idx);
                    ui.close();
                }
                if ui.button("Duplicate").on_hover_text("Clone this tileset").clicked() {
                    action = TilesetAction::DuplicateTileset(idx);
                    ui.close();
                }
                if ui.button("Export PNG...").on_hover_text("Save tileset image to file").clicked() {
                    action = TilesetAction::ExportTileset(idx);
                    ui.close();
                }
                if ui.button("Paint...").on_hover_text("Open pixel paint editor for this tileset").clicked() {
                    action = TilesetAction::OpenPaintEditor;
                    ui.close();
                }
                ui.separator();
                if ui.button("Remove").on_hover_text("Remove this tileset").clicked() {
                    action = TilesetAction::RemoveTileset(idx);
                    ui.close();
                }
                if ui.button("Remove Unused").on_hover_text("Remove tilesets not referenced by any object").clicked() {
                    action = TilesetAction::RemoveUnusedTilesets;
                    ui.close();
                }
            });
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
                    return action;
                }

                // Scale tileset to fit available width, then apply zoom multiplier.
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

    // Material settings (collapsible)
    if let Some(active_idx) = scene.active_tileset
        && let Some(tileset) = scene.tilesets.get_mut(active_idx)
    {
        ui.separator();
        egui::CollapsingHeader::new("Material").default_open(false).show(ui, |ui| {
            let mat = &mut tileset.material;
            let mut changed = false;

            ui.horizontal(|ui| {
                ui.label("Filter:");
                let prev = mat.filter;
                egui::ComboBox::from_id_salt("mat_filter")
                    .selected_text(format!("{:?}", mat.filter))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut mat.filter, FilterMode::Nearest, "Nearest");
                        ui.selectable_value(&mut mat.filter, FilterMode::Linear, "Linear");
                    });
                if mat.filter != prev { changed = true; }
            });

            ui.horizontal(|ui| {
                ui.label("Wrap:");
                let prev = mat.wrap;
                egui::ComboBox::from_id_salt("mat_wrap")
                    .selected_text(format!("{:?}", mat.wrap))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut mat.wrap, WrapMode::ClampToEdge, "Clamp");
                        ui.selectable_value(&mut mat.wrap, WrapMode::Repeat, "Repeat");
                        ui.selectable_value(&mut mat.wrap, WrapMode::MirroredRepeat, "Mirror");
                    });
                if mat.wrap != prev { changed = true; }
            });

            ui.horizontal(|ui| {
                ui.label("Alpha:");
                let prev = mat.alpha_mode;
                egui::ComboBox::from_id_salt("mat_alpha")
                    .selected_text(format!("{:?}", mat.alpha_mode))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut mat.alpha_mode, AlphaMode::AlphaTest, "Alpha Test");
                        ui.selectable_value(&mut mat.alpha_mode, AlphaMode::AlphaBlend, "Alpha Blend");
                        ui.selectable_value(&mut mat.alpha_mode, AlphaMode::Opaque, "Opaque");
                    });
                if mat.alpha_mode != prev { changed = true; }
            });

            if mat.alpha_mode == AlphaMode::AlphaTest {
                ui.horizontal(|ui| {
                    ui.label("Cutoff:");
                    ui.add(egui::DragValue::new(&mut mat.alpha_cutoff).range(0.0..=1.0).speed(0.01));
                });
            }

            ui.checkbox(&mut mat.decal, "Decal overlay");

            if changed {
                action = TilesetAction::RebuildMaterial(active_idx);
            }
        });
    }

    // Palette section (collapsible)
    ui.separator();
    egui::CollapsingHeader::new("Palette").default_open(false).show(ui, |ui| {
        // Palette selector
        ui.horizontal(|ui| {
            let current_name = scene.active_palette
                .and_then(|i| scene.palettes.get(i))
                .map(|p| p.name.clone())
                .unwrap_or_else(|| "None".to_string());

            egui::ComboBox::from_id_salt("palette_selector")
                .selected_text(&current_name)
                .show_ui(ui, |ui| {
                    if ui.selectable_label(scene.active_palette.is_none(), "None").clicked() {
                        scene.active_palette = None;
                    }
                    for (i, pal) in scene.palettes.iter().enumerate() {
                        let sel = scene.active_palette == Some(i);
                        if ui.selectable_label(sel, &pal.name).clicked() {
                            scene.active_palette = Some(i);
                        }
                    }
                });

            if ui.small_button("+").on_hover_text("New palette").clicked() {
                let n = scene.palettes.len() + 1;
                scene.palettes.push(crate::tile::palette::Palette::new(format!("Palette {n}")));
                scene.active_palette = Some(scene.palettes.len() - 1);
            }
        });

        // Active palette controls
        if let Some(pal_idx) = scene.active_palette
            && let Some(palette) = scene.palettes.get_mut(pal_idx)
        {
                ui.horizontal(|ui| {
                    ui.label("Mode:");
                    egui::ComboBox::from_id_salt("palette_mode")
                        .selected_text(format!("{:?}", palette.mode))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut palette.mode, PaletteMode::Random, "Random");
                            ui.selectable_value(&mut palette.mode, PaletteMode::Sequence, "Sequence");
                        });
                });

                ui.horizontal(|ui| {
                    ui.checkbox(&mut palette.random_rotation, "Rand Rot");
                    ui.checkbox(&mut palette.random_flip_h, "Rand FlipH");
                    ui.checkbox(&mut palette.random_flip_v, "Rand FlipV");
                });

                // Add current tile to palette
                if let Some(ts_idx) = scene.active_tileset
                    && ui.button("Add Current Tile").clicked()
                {
                    let col = draw_state.selected_tile.0;
                    let row = draw_state.selected_tile.1;
                    palette.add_entry(ts_idx, col, row);
                }

                // Show entries with weight sliders
                let mut remove_idx = None;
                for (i, entry) in palette.entries.iter_mut().enumerate() {
                    ui.horizontal(|ui| {
                        ui.label(format!("T{}({},{})", entry.tileset_index, entry.col, entry.row));
                        ui.add(egui::DragValue::new(&mut entry.weight).range(0.01..=10.0).speed(0.05).prefix("w:"));
                        if ui.small_button("x").clicked() {
                            remove_idx = Some(i);
                        }
                    });
                }
                if let Some(idx) = remove_idx {
                    palette.entries.remove(idx);
                }

                ui.horizontal(|ui| {
                    if ui.small_button("Normalize").on_hover_text("Normalize weights to sum to 1.0").clicked() {
                        palette.normalize_weights();
                    }
                    if ui.small_button("Clear").on_hover_text("Remove all entries").clicked() {
                        palette.entries.clear();
                    }
                    if ui.small_button("Delete").on_hover_text("Delete this palette").clicked() {
                        // Will be handled after the borrow ends
                    }
                });

                ui.label(format!("{} entries", palette.entries.len()));
        }
    });

    action
}
