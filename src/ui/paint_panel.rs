use crate::paint::{PaintState, PaintTool};

/// Actions the paint panel wants the app to execute.
pub enum PaintAction {
    None,
    /// Paint editor needs to sync pixels back to the tileset GPU texture.
    SyncToGpu,
}

/// Draw the paint editor as a floating window.
pub fn draw_paint_panel(
    ctx: &egui::Context,
    paint: &mut PaintState,
) -> PaintAction {
    if !paint.open {
        return PaintAction::None;
    }

    let mut action = PaintAction::None;
    let mut open = true;

    egui::Window::new("Paint Editor")
        .id(egui::Id::new("paint_editor_window"))
        .open(&mut open)
        .resizable(true)
        .default_size([500.0, 500.0])
        .show(ctx, |ui| {
            action = draw_paint_content(ui, paint);
        });

    if !open {
        paint.open = false;
    }

    action
}

fn draw_paint_content(
    ui: &mut egui::Ui,
    paint: &mut PaintState,
) -> PaintAction {
    let mut action = PaintAction::None;

    if paint.width == 0 || paint.height == 0 {
        ui.label("No tileset loaded in paint editor.");
        return action;
    }

    // Toolbar row
    ui.horizontal(|ui| {
        // Tool selector
        let tools = [
            (PaintTool::Pencil, "Pencil"),
            (PaintTool::Eraser, "Eraser"),
            (PaintTool::Eyedropper, "Eyedropper"),
            (PaintTool::Bucket, "Bucket"),
        ];
        for (tool, name) in &tools {
            if ui.selectable_label(paint.tool == *tool, *name).clicked() {
                paint.tool = *tool;
            }
        }

        ui.separator();

        // Brush size
        ui.label("Size:");
        ui.add(egui::DragValue::new(&mut paint.brush_size).range(1..=32).speed(0.2));

        ui.separator();

        // Undo/redo
        if ui.add_enabled(paint.can_undo(), egui::Button::new("Undo")).clicked() {
            paint.undo();
            action = PaintAction::SyncToGpu;
        }
        if ui.add_enabled(paint.can_redo(), egui::Button::new("Redo")).clicked() {
            paint.redo();
            action = PaintAction::SyncToGpu;
        }
    });

    // Color pickers
    ui.horizontal(|ui| {
        ui.label("Primary:");
        let mut pf = color_u8_to_f32(paint.primary_color);
        if ui.color_edit_button_rgba_unmultiplied(&mut pf).changed() {
            paint.primary_color = color_f32_to_u8(pf);
        }
        ui.label("Secondary:");
        let mut sf = color_u8_to_f32(paint.secondary_color);
        if ui.color_edit_button_rgba_unmultiplied(&mut sf).changed() {
            paint.secondary_color = color_f32_to_u8(sf);
        }
        if ui.small_button("Swap").clicked() {
            std::mem::swap(&mut paint.primary_color, &mut paint.secondary_color);
        }
    });

    // Zoom controls
    ui.horizontal(|ui| {
        ui.label(format!("Zoom: {:.0}x", paint.zoom));
        if ui.small_button("-").clicked() {
            paint.zoom = (paint.zoom / 2.0).max(1.0);
        }
        if ui.small_button("+").clicked() {
            paint.zoom = (paint.zoom * 2.0).min(32.0);
        }
        if ui.small_button("1x").clicked() {
            paint.zoom = 1.0;
        }
    });

    ui.separator();

    // Canvas
    let display_w = paint.width as f32 * paint.zoom;
    let display_h = paint.height as f32 * paint.zoom;

    egui::ScrollArea::both().show(ui, |ui| {
        let (response, painter) = ui.allocate_painter(
            egui::vec2(display_w, display_h),
            egui::Sense::click_and_drag(),
        );
        let rect = response.rect;

        // Draw checkerboard background (transparency indicator)
        let check_size = 8.0 * paint.zoom.min(4.0);
        let cols = (rect.width() / check_size).ceil() as u32;
        let rows = (rect.height() / check_size).ceil() as u32;
        for cy in 0..rows {
            for cx in 0..cols {
                let dark = (cx + cy) % 2 == 0;
                let color = if dark {
                    egui::Color32::from_gray(180)
                } else {
                    egui::Color32::from_gray(220)
                };
                let cell_rect = egui::Rect::from_min_size(
                    egui::pos2(rect.left() + cx as f32 * check_size, rect.top() + cy as f32 * check_size),
                    egui::vec2(check_size, check_size),
                );
                painter.rect_filled(cell_rect.intersect(rect), 0.0, color);
            }
        }

        // Draw pixels as colored rectangles
        let pixel_w = rect.width() / paint.width as f32;
        let pixel_h = rect.height() / paint.height as f32;

        for py in 0..paint.height {
            for px in 0..paint.width {
                let idx = ((py * paint.width + px) * 4) as usize;
                if idx + 3 >= paint.pixels.len() { continue; }
                let r = paint.pixels[idx];
                let g = paint.pixels[idx + 1];
                let b = paint.pixels[idx + 2];
                let a = paint.pixels[idx + 3];
                if a == 0 { continue; } // Skip fully transparent pixels (checkerboard shows through)

                let color = egui::Color32::from_rgba_unmultiplied(r, g, b, a);
                let pixel_rect = egui::Rect::from_min_size(
                    egui::pos2(rect.left() + px as f32 * pixel_w, rect.top() + py as f32 * pixel_h),
                    egui::vec2(pixel_w, pixel_h),
                );
                painter.rect_filled(pixel_rect, 0.0, color);
            }
        }

        // Handle scroll-to-zoom
        let hover_pos = ui.input(|i| i.pointer.hover_pos());
        if let Some(hp) = hover_pos
            && rect.contains(hp)
        {
            let scroll = ui.input(|i| i.raw_scroll_delta.y);
            if scroll != 0.0 {
                if scroll > 0.0 {
                    paint.zoom = (paint.zoom * 2.0).min(32.0);
                } else {
                    paint.zoom = (paint.zoom / 2.0).max(1.0);
                }
            }
        }

        // Convert mouse position to pixel coordinates
        let to_pixel = |pos: egui::Pos2| -> (i32, i32) {
            let x = ((pos.x - rect.left()) / pixel_w) as i32;
            let y = ((pos.y - rect.top()) / pixel_h) as i32;
            (x, y)
        };

        // Handle paint input
        if response.drag_started() {
            paint.begin_stroke();

            if let Some(pos) = response.interact_pointer_pos() {
                let (px, py) = to_pixel(pos);
                apply_tool(paint, px, py);
                action = PaintAction::SyncToGpu;
            }
        }

        if response.dragged()
            && let Some(pos) = response.interact_pointer_pos()
        {
            let (px, py) = to_pixel(pos);
            apply_tool(paint, px, py);
            action = PaintAction::SyncToGpu;
        }

        if response.drag_stopped() {
            paint.end_stroke();
        }

        // Single click (for eyedropper / bucket)
        if response.clicked()
            && let Some(pos) = response.interact_pointer_pos()
        {
            let (px, py) = to_pixel(pos);
            match paint.tool {
                PaintTool::Eyedropper => {
                    if px >= 0 && py >= 0 {
                        paint.primary_color = paint.sample(px as u32, py as u32);
                    }
                }
                PaintTool::Bucket => {
                    paint.begin_stroke();
                    if px >= 0 && py >= 0 {
                        paint.bucket_fill(px as u32, py as u32, paint.primary_color);
                    }
                    paint.end_stroke();
                    action = PaintAction::SyncToGpu;
                }
                _ => {}
            }
        }

        // Draw grid lines when zoomed in enough
        if paint.zoom >= 4.0 {
            let grid_color = egui::Color32::from_rgba_premultiplied(0, 0, 0, 30);
            for px in 0..=paint.width {
                let x = rect.left() + px as f32 * pixel_w;
                painter.line_segment(
                    [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
                    egui::Stroke::new(0.5, grid_color),
                );
            }
            for py in 0..=paint.height {
                let y = rect.top() + py as f32 * pixel_h;
                painter.line_segment(
                    [egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)],
                    egui::Stroke::new(0.5, grid_color),
                );
            }
        }
    });

    action
}

fn apply_tool(paint: &mut PaintState, px: i32, py: i32) {
    match paint.tool {
        PaintTool::Pencil => {
            let color = paint.primary_color;
            paint.paint(px, py, color);
        }
        PaintTool::Eraser => {
            paint.erase(px, py);
        }
        PaintTool::Eyedropper => {
            if px >= 0 && py >= 0 {
                paint.primary_color = paint.sample(px as u32, py as u32);
            }
        }
        PaintTool::Bucket => {
            // Bucket handled on click, not drag
        }
    }
}

fn color_u8_to_f32(c: [u8; 4]) -> [f32; 4] {
    [c[0] as f32 / 255.0, c[1] as f32 / 255.0, c[2] as f32 / 255.0, c[3] as f32 / 255.0]
}

fn color_f32_to_u8(c: [f32; 4]) -> [u8; 4] {
    [(c[0] * 255.0) as u8, (c[1] * 255.0) as u8, (c[2] * 255.0) as u8, (c[3] * 255.0) as u8]
}
