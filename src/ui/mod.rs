mod tools_panel;
mod properties_panel;
mod layers_panel;
pub mod tileset_panel;

use crate::scene::{Scene, Layer};
use crate::tools::ToolMode;
use crate::tools::draw::DrawState;
use crate::tools::edit::EditState;
use crate::history::History;

/// Actions the UI wants the app to execute (can't borrow mutably inside egui closures).
pub enum UiAction {
    None,
    NewScene,
    Undo,
    Redo,
    Quit,
    LoadTileset,
    SaveScene,
    OpenScene,
    ExportObj,
    ConfirmTilesetLoad,
    ToggleWireframe,
    SaveSceneAs,
    ExportGlb,
    ConfirmNewScene,
    // Edit operations triggered by UI buttons
    RotateCW,
    RotateCCW,
    FlipNormals,
    ExtrudeFaces,
    Retile,
    SubdivideFaces,
    DeleteSelection,
    SelectAll,
    DeselectAll,
    InvertSelection,
}

/// Draw all egui UI panels. Called each frame within egui context.
#[allow(clippy::too_many_arguments)]
pub fn draw_ui(
    ctx: &egui::Context,
    scene: &mut Scene,
    tool_mode: &mut ToolMode,
    draw_state: &mut DrawState,
    edit_state: &mut EditState,
    history: &History,
    wireframe: bool,
    bg_color: &mut [f32; 3],
    has_unsaved_changes: bool,
) -> UiAction {
    let mut action = UiAction::None;

    // Menu bar
    egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("New").clicked() {
                    if has_unsaved_changes {
                        action = UiAction::ConfirmNewScene;
                    } else {
                        action = UiAction::NewScene;
                    }
                    ui.close();
                }
                if ui.button("Open...  Ctrl+O").clicked() {
                    action = UiAction::OpenScene;
                    ui.close();
                }
                if ui.button("Save  Ctrl+S").clicked() {
                    action = UiAction::SaveScene;
                    ui.close();
                }
                if ui.button("Save As...").clicked() {
                    action = UiAction::SaveSceneAs;
                    ui.close();
                }
                ui.separator();
                if ui.button("Load Tileset...").clicked() {
                    action = UiAction::LoadTileset;
                    ui.close();
                }
                ui.separator();
                ui.menu_button("Export", |ui| {
                    if ui.button("Wavefront OBJ (.obj)").clicked() {
                        action = UiAction::ExportObj;
                        ui.close();
                    }
                    if ui.button("glTF Binary (.glb)").clicked() {
                        action = UiAction::ExportGlb;
                        ui.close();
                    }
                });
                ui.separator();
                if ui.button("Quit").clicked() {
                    action = UiAction::Quit;
                    ui.close();
                }
            });
            ui.menu_button("Edit", |ui| {
                let undo_label = if history.can_undo() { "Undo  Ctrl+Z" } else { "Undo" };
                if ui.add_enabled(history.can_undo(), egui::Button::new(undo_label)).clicked() {
                    action = UiAction::Undo;
                    ui.close();
                }
                let redo_label = if history.can_redo() { "Redo  Ctrl+Y" } else { "Redo" };
                if ui.add_enabled(history.can_redo(), egui::Button::new(redo_label)).clicked() {
                    action = UiAction::Redo;
                    ui.close();
                }
            });
            ui.menu_button("View", |ui| {
                if ui.button("Perspective / Orthographic  Num5").clicked() {
                    ui.close();
                }
                let wf_label = if wireframe { "Wireframe [ON]  Z" } else { "Wireframe  Z" };
                if ui.button(wf_label).clicked() {
                    action = UiAction::ToggleWireframe;
                    ui.close();
                }
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label("Background:");
                    ui.color_edit_button_rgb(bg_color);
                });
            });
        });
    });

    // Tools panel (left)
    let tools_action = tools_panel::draw_tools_panel(ctx, tool_mode, draw_state, edit_state, scene.crosshair_pos);
    if !matches!(tools_action, UiAction::None) {
        action = tools_action;
    }

    // Layers + Properties panel (right)
    let layer_action = layers_panel::draw_layers_panel(ctx, scene, edit_state);
    match layer_action {
        layers_panel::LayerAction::AddLayer => {
            let n = scene.layers.len() + 1;
            scene.layers.push(Layer {
                name: format!("Layer {n}"),
                visible: true,
                objects: Vec::new(),
            });
        }
        layers_panel::LayerAction::DeleteLayer(i) => {
            if scene.layers.len() > 1 {
                scene.layers.remove(i);
                if scene.active_layer >= scene.layers.len() {
                    scene.active_layer = scene.layers.len() - 1;
                }
            }
        }
        layers_panel::LayerAction::DuplicateLayer(i) => {
            if let Some(layer) = scene.layers.get(i) {
                let mut dup = Layer {
                    name: format!("{} (copy)", layer.name),
                    visible: layer.visible,
                    objects: Vec::new(),
                };
                for obj in &layer.objects {
                    let mut new_obj = crate::scene::Object::new(format!("{} (copy)", obj.name));
                    new_obj.faces = obj.faces.clone();
                    dup.objects.push(new_obj);
                }
                scene.layers.insert(i + 1, dup);
            }
        }
        layers_panel::LayerAction::None => {}
    }

    // Tileset panel (bottom, above status bar) — visible in both modes for retile support
    {
        let tileset_action = tileset_panel::draw_tileset_panel(ctx, scene, draw_state);
        match tileset_action {
            tileset_panel::TilesetAction::LoadTileset => {
                action = UiAction::LoadTileset;
            }
            tileset_panel::TilesetAction::None => {}
        }
    }

    // Status bar
    egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
        ui.horizontal(|ui| {
            // Mode + tool name
            match tool_mode {
                ToolMode::Draw => {
                    ui.label(format!("Draw: {:?}", draw_state.tool));
                }
                ToolMode::Edit => {
                    ui.label(format!("Edit: {:?} / {:?}", edit_state.selection_level, edit_state.gizmo_mode));
                }
            }
            ui.separator();
            ui.label(format!("Grid: {}", scene.grid_cell_size));
            ui.separator();
            // Crosshair position
            let cp = scene.crosshair_pos;
            ui.label(format!("Pos: ({:.1}, {:.1}, {:.1})", cp.x, cp.y, cp.z));
            ui.separator();
            let total_faces: usize = scene.layers.iter()
                .flat_map(|l| &l.objects)
                .map(|o| o.faces.len())
                .sum();
            ui.label(format!("Faces: {total_faces}"));
            ui.separator();
            let sel = &edit_state.selection;
            let sel_count = sel.faces.len() + sel.objects.len() + sel.vertices.len();
            if sel_count > 0 {
                ui.label(format!("Selected: {sel_count}"));
                ui.separator();
            }
            if wireframe {
                ui.label("Wireframe");
                ui.separator();
            }
        });
    });

    action
}
