use crate::scene::Scene;
use crate::tools::ToolMode;

/// Draw all egui UI panels. Called each frame within egui context.
pub fn draw_ui(ctx: &egui::Context, scene: &mut Scene, tool_mode: &mut ToolMode) {
    egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("New").clicked() {
                    *scene = Scene::new();
                    ui.close();
                }
                ui.separator();
                if ui.button("Quit").clicked() {
                    std::process::exit(0);
                }
            });
            ui.menu_button("View", |ui| {
                if ui.button("Perspective / Orthographic").clicked() {
                    // Toggled via camera — will wire up later
                    ui.close();
                }
            });
        });
    });

    egui::SidePanel::left("tools_panel").default_width(180.0).show(ctx, |ui| {
        ui.heading("Mode");
        ui.horizontal(|ui| {
            ui.selectable_value(tool_mode, ToolMode::Draw, "Draw");
            ui.selectable_value(tool_mode, ToolMode::Edit, "Edit");
        });
        ui.separator();

        match tool_mode {
            ToolMode::Draw => {
                ui.heading("Draw Tools");
                ui.label("1 - Tile");
                ui.label("2 - Sticky");
                ui.label("3 - Block");
                ui.label("4 - Primitive");
                ui.label("5 - Vertex Color");
            }
            ToolMode::Edit => {
                ui.heading("Edit Tools");
                ui.label("G - Translate");
                ui.label("R - Rotate");
                ui.label("S - Scale");
                ui.label("E - Extrude");
            }
        }

        ui.separator();
        ui.heading("Crosshair");
        ui.label(format!(
            "({:.1}, {:.1}, {:.1})",
            scene.crosshair_pos.x, scene.crosshair_pos.y, scene.crosshair_pos.z
        ));
    });

    egui::SidePanel::right("layers_panel").default_width(160.0).show(ctx, |ui| {
        ui.heading("Layers");
        for (_i, layer) in scene.layers.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.checkbox(&mut layer.visible, "");
                ui.label(&layer.name);
                ui.label(format!("({} obj)", layer.objects.len()));
            });
        }
        if ui.button("+ Add Layer").clicked() {
            let n = scene.layers.len() + 1;
            scene.layers.push(crate::scene::Layer {
                name: format!("Layer {n}"),
                visible: true,
                objects: Vec::new(),
            });
        }
    });

    egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.label(format!("Mode: {:?}", tool_mode));
            ui.separator();
            let total_faces: usize = scene.layers.iter()
                .flat_map(|l| &l.objects)
                .map(|o| o.faces.len())
                .sum();
            ui.label(format!("Faces: {total_faces}"));
        });
    });
}
