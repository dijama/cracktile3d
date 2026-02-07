use crate::scene::Scene;
use crate::tools::edit::EditState;

/// UI action returned from the layers panel for the caller to execute.
pub enum LayerAction {
    None,
    AddLayer,
    DeleteLayer(usize),
    DuplicateLayer(usize),
}

/// Draw the layers panel (right side).
pub fn draw_layers_panel(
    ctx: &egui::Context,
    scene: &mut Scene,
    edit_state: &mut EditState,
) -> LayerAction {
    let mut action = LayerAction::None;

    egui::SidePanel::right("layers_panel").default_width(200.0).show(ctx, |ui| {
        ui.heading("Layers");

        for i in 0..scene.layers.len() {
            let is_active = scene.active_layer == i;
            let layer_name = scene.layers[i].name.clone();
            let obj_count = scene.layers[i].objects.len();
            let face_count: usize = scene.layers[i].objects.iter().map(|o| o.faces.len()).sum();
            let visible = &mut scene.layers[i].visible;

            let response = ui.horizontal(|ui| {
                ui.checkbox(visible, "");
                let resp = ui.selectable_label(is_active, &layer_name);
                if resp.clicked() {
                    scene.active_layer = i;
                }

                ui.small(format!("({obj_count} obj, {face_count} f)"));
                resp
            }).inner;

            // Context menu on right-click
            response.context_menu(|ui| {
                ui.horizontal(|ui| {
                    ui.label("Name:");
                    ui.text_edit_singleline(&mut scene.layers[i].name);
                });
                ui.separator();
                if ui.button("Duplicate").clicked() {
                    action = LayerAction::DuplicateLayer(i);
                    ui.close();
                }
                if ui.button("Delete").clicked() {
                    action = LayerAction::DeleteLayer(i);
                    ui.close();
                }
            });

            // Object tree within each layer (collapsible)
            if !scene.layers[i].objects.is_empty() {
                let id = ui.make_persistent_id(format!("layer_{i}_objects"));
                egui::CollapsingHeader::new("Objects")
                    .id_salt(id)
                    .default_open(is_active)
                    .show(ui, |ui| {
                        for oi in 0..scene.layers[i].objects.len() {
                            let obj_name = scene.layers[i].objects[oi].name.clone();
                            let obj_faces = scene.layers[i].objects[oi].faces.len();
                            let is_selected = edit_state.selection.objects.contains(&(i, oi));

                            ui.horizontal(|ui| {
                                ui.add_space(16.0);
                                let resp = ui.selectable_label(is_selected, format!("{obj_name} ({obj_faces}f)"));
                                if resp.clicked() {
                                    if !ui.input(|inp| inp.modifiers.shift) {
                                        edit_state.selection.clear();
                                    }
                                    if is_selected {
                                        edit_state.selection.objects.retain(|&(li, o)| li != i || o != oi);
                                    } else {
                                        edit_state.selection.objects.push((i, oi));
                                    }
                                }

                                // Object context menu
                                resp.context_menu(|ui| {
                                    ui.horizontal(|ui| {
                                        ui.label("Name:");
                                        ui.text_edit_singleline(&mut scene.layers[i].objects[oi].name);
                                    });
                                });
                            });
                        }
                    });
            }
        }

        ui.separator();
        if ui.button("+ Add Layer").clicked() {
            action = LayerAction::AddLayer;
        }

        // Properties sub-section
        ui.separator();
        ui.heading("Properties");
        super::properties_panel::draw_properties_panel(ui, scene, edit_state);
    });

    action
}
