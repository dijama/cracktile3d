use crate::tools::ToolMode;
use crate::tools::draw::{DrawState, DrawTool, PrimitiveShape};
use crate::tools::edit::{EditState, SelectionLevel, GizmoMode};
use crate::ui::UiAction;

/// Draw the tools panel (left side). Returns a UiAction if an edit operation button was clicked.
pub fn draw_tools_panel(
    ctx: &egui::Context,
    tool_mode: &mut ToolMode,
    draw_state: &mut DrawState,
    edit_state: &mut EditState,
    crosshair_pos: glam::Vec3,
) -> UiAction {
    let mut action = UiAction::None;
    egui::SidePanel::left("tools_panel").default_width(180.0).show(ctx, |ui| {
        ui.heading("Mode");
        ui.horizontal(|ui| {
            ui.selectable_value(tool_mode, ToolMode::Draw, "Draw");
            ui.selectable_value(tool_mode, ToolMode::Edit, "Edit");
        });
        ui.small("Tab to toggle");
        ui.separator();

        match tool_mode {
            ToolMode::Draw => {
                draw_draw_tools(ui, draw_state);
            }
            ToolMode::Edit => {
                action = draw_edit_tools(ui, edit_state);
            }
        }

        ui.separator();
        ui.heading("Crosshair");
        ui.label(format!(
            "({:.1}, {:.1}, {:.1})",
            crosshair_pos.x, crosshair_pos.y, crosshair_pos.z
        ));
        ui.small("WASD + Q/E to move");
    });
    action
}

fn draw_draw_tools(ui: &mut egui::Ui, draw_state: &mut DrawState) {
    ui.heading("Draw Tools");
    let tools = [
        (DrawTool::Tile, "Tile", "1"),
        (DrawTool::Sticky, "Sticky", "2"),
        (DrawTool::Block, "Block", "3"),
        (DrawTool::Primitive, "Primitive", "4"),
        (DrawTool::VertexColor, "Vtx Color", "5"),
    ];
    for (tool, label, key) in &tools {
        let selected = draw_state.tool == *tool;
        if ui.selectable_label(selected, format!("[{key}] {label}")).clicked() {
            draw_state.tool = *tool;
        }
    }
    ui.separator();
    match draw_state.tool {
        DrawTool::Tile => {
            ui.small("Click: place tile on grid/face");
            ui.small("Right click: erase tile");
        }
        DrawTool::Sticky => {
            ui.small("Click face edge: extend tile");
            ui.small("Right click: erase tile");
        }
        DrawTool::Block => {
            ui.small("Click: place 6-face cube");
            ui.small("Right click: erase tile");
        }
        DrawTool::Primitive => {
            ui.heading("Shape");
            ui.horizontal(|ui| {
                ui.selectable_value(&mut draw_state.selected_primitive, PrimitiveShape::Box, "Box");
                ui.selectable_value(&mut draw_state.selected_primitive, PrimitiveShape::Cylinder, "Cyl");
                ui.selectable_value(&mut draw_state.selected_primitive, PrimitiveShape::Cone, "Cone");
            });
            ui.horizontal(|ui| {
                ui.selectable_value(&mut draw_state.selected_primitive, PrimitiveShape::Sphere, "Sphere");
                ui.selectable_value(&mut draw_state.selected_primitive, PrimitiveShape::Wedge, "Wedge");
            });
            ui.small("Click: place primitive shape");
            ui.small("Right click: erase tile");
        }
        DrawTool::VertexColor => {
            ui.heading("Paint Color");
            ui.color_edit_button_rgba_unmultiplied(&mut draw_state.paint_color);
            ui.small("Click face: paint all vertices");
            ui.small("Shift+click: paint closest vertex");
        }
    }
    ui.separator();
    ui.small("[ / ]: change grid size");
}

fn draw_edit_tools(ui: &mut egui::Ui, edit_state: &mut EditState) -> UiAction {
    let mut action = UiAction::None;

    ui.heading("Selection Level");
    ui.horizontal(|ui| {
        ui.selectable_value(&mut edit_state.selection_level, SelectionLevel::Object, "Obj");
        ui.selectable_value(&mut edit_state.selection_level, SelectionLevel::Face, "Face");
        ui.selectable_value(&mut edit_state.selection_level, SelectionLevel::Vertex, "Vtx");
        ui.selectable_value(&mut edit_state.selection_level, SelectionLevel::Edge, "Edge");
    });

    ui.separator();
    ui.heading("Transform");
    ui.horizontal(|ui| {
        ui.selectable_value(&mut edit_state.gizmo_mode, GizmoMode::Translate, "Move");
        ui.selectable_value(&mut edit_state.gizmo_mode, GizmoMode::Rotate, "Rotate");
        ui.selectable_value(&mut edit_state.gizmo_mode, GizmoMode::Scale, "Scale");
    });

    ui.separator();
    let sel = &edit_state.selection;
    let count = sel.faces.len() + sel.objects.len() + sel.vertices.len();
    let has_selection = count > 0;
    let has_faces = !sel.faces.is_empty();
    ui.label(format!("Selected: {count}"));

    ui.separator();
    ui.heading("Operations");
    ui.horizontal(|ui| {
        if ui.add_enabled(has_selection, egui::Button::new("Rot CW")).clicked() {
            action = UiAction::RotateCW;
        }
        if ui.add_enabled(has_selection, egui::Button::new("Rot CCW")).clicked() {
            action = UiAction::RotateCCW;
        }
    });
    ui.horizontal(|ui| {
        if ui.add_enabled(has_selection, egui::Button::new("Flip")).clicked() {
            action = UiAction::FlipNormals;
        }
        if ui.add_enabled(has_faces, egui::Button::new("Extrude")).clicked() {
            action = UiAction::ExtrudeFaces;
        }
    });
    ui.horizontal(|ui| {
        if ui.add_enabled(has_faces, egui::Button::new("Retile")).clicked() {
            action = UiAction::Retile;
        }
        if ui.add_enabled(has_faces, egui::Button::new("Subdivide")).clicked() {
            action = UiAction::SubdivideFaces;
        }
    });
    if ui.add_enabled(has_selection, egui::Button::new("Delete")).clicked() {
        action = UiAction::DeleteSelection;
    }

    ui.separator();
    ui.heading("Select");
    ui.horizontal(|ui| {
        if ui.button("All").clicked() { action = UiAction::SelectAll; }
        if ui.button("None").clicked() { action = UiAction::DeselectAll; }
        if ui.button("Invert").clicked() { action = UiAction::InvertSelection; }
    });

    ui.separator();
    ui.small("Click: select, Shift+click: add");
    ui.small("Drag: marquee select");
    ui.small("Arrows: move | R: rotate | F: flip");
    ui.small("+/-: scale | E: extrude | T: retile");
    ui.small("Ctrl+C/V: copy/paste | Z: wireframe");

    action
}
