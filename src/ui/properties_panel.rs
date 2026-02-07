use crate::scene::Scene;
use crate::tools::edit::EditState;

/// Draw the properties panel (right side, below layers).
/// Editable properties push dirty objects to `scene.dirty_objects` for GPU rebuild.
pub fn draw_properties_panel(ui: &mut egui::Ui, scene: &mut Scene, edit_state: &EditState) {
    let sel = &edit_state.selection;
    if sel.is_empty() {
        ui.label("No selection");
        return;
    }

    // Show face properties
    if !sel.faces.is_empty() {
        ui.label(format!("{} face(s) selected", sel.faces.len()));

        if sel.faces.len() == 1 {
            let (li, oi, fi) = sel.faces[0];
            if let Some(face) = scene.layers.get_mut(li)
                .and_then(|l| l.objects.get_mut(oi))
                .and_then(|o| o.faces.get_mut(fi))
            {
                // Editable vertex positions
                egui::CollapsingHeader::new("Vertices").default_open(true).show(ui, |ui| {
                    let mut changed = false;
                    for vi in 0..4 {
                        ui.horizontal(|ui| {
                            ui.label(format!("v{vi}:"));
                            changed |= ui.add(egui::DragValue::new(&mut face.positions[vi].x).speed(0.05).prefix("x:")).changed();
                            changed |= ui.add(egui::DragValue::new(&mut face.positions[vi].y).speed(0.05).prefix("y:")).changed();
                            changed |= ui.add(egui::DragValue::new(&mut face.positions[vi].z).speed(0.05).prefix("z:")).changed();
                        });
                    }
                    if changed {
                        scene.dirty_objects.push((li, oi));
                    }
                });

                // Show normal (read-only)
                let n = face.normal();
                ui.label(format!("Normal: ({:.2}, {:.2}, {:.2})", n.x, n.y, n.z));

                // Editable UVs
                egui::CollapsingHeader::new("UVs").show(ui, |ui| {
                    let mut changed = false;
                    for vi in 0..4 {
                        ui.horizontal(|ui| {
                            ui.label(format!("v{vi}:"));
                            changed |= ui.add(egui::DragValue::new(&mut face.uvs[vi].x).speed(0.01).prefix("u:")).changed();
                            changed |= ui.add(egui::DragValue::new(&mut face.uvs[vi].y).speed(0.01).prefix("v:")).changed();
                        });
                    }
                    if changed {
                        scene.dirty_objects.push((li, oi));
                    }
                });

                // Editable vertex colors
                egui::CollapsingHeader::new("Colors").show(ui, |ui| {
                    let mut changed = false;
                    for vi in 0..4 {
                        let c = &mut face.colors[vi];
                        let mut rgba = [c.x, c.y, c.z, c.w];
                        ui.horizontal(|ui| {
                            ui.label(format!("v{vi}:"));
                            if ui.color_edit_button_rgba_unmultiplied(&mut rgba).changed() {
                                *c = glam::Vec4::new(rgba[0], rgba[1], rgba[2], rgba[3]);
                                changed = true;
                            }
                        });
                    }
                    if changed {
                        scene.dirty_objects.push((li, oi));
                    }
                });
            }
        }
    }

    // Show object selection info
    if !sel.objects.is_empty() {
        ui.label(format!("{} object(s) selected", sel.objects.len()));
        for &(li, oi) in &sel.objects {
            if let Some(obj) = scene.layers.get(li).and_then(|l| l.objects.get(oi)) {
                ui.label(format!("  {} ({} faces)", obj.name, obj.faces.len()));
            }
        }
    }

    // Show vertex selection info
    if !sel.vertices.is_empty() {
        ui.label(format!("{} vertex(es) selected", sel.vertices.len()));
        if sel.vertices.len() == 1 {
            let (li, oi, fi, vi) = sel.vertices[0];
            if let Some(face) = scene.layers.get_mut(li)
                .and_then(|l| l.objects.get_mut(oi))
                .and_then(|o| o.faces.get_mut(fi))
            {
                let mut changed = false;
                ui.horizontal(|ui| {
                    ui.label("Pos:");
                    changed |= ui.add(egui::DragValue::new(&mut face.positions[vi].x).speed(0.05).prefix("x:")).changed();
                    changed |= ui.add(egui::DragValue::new(&mut face.positions[vi].y).speed(0.05).prefix("y:")).changed();
                    changed |= ui.add(egui::DragValue::new(&mut face.positions[vi].z).speed(0.05).prefix("z:")).changed();
                });
                if changed {
                    scene.dirty_objects.push((li, oi));
                }
            }
        }
    }
}
