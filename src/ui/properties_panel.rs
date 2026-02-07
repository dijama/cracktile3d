use glam::{Vec2, Vec3, Vec4};
use crate::scene::Scene;

use crate::tools::edit::EditState;

/// Snapshot of a face's state before editing, for deferred undo commit.
pub struct PropertyEditSnapshot {
    pub face: (usize, usize, usize),
    pub positions: [Vec3; 4],
    pub uvs: [Vec2; 4],
    pub colors: [Vec4; 4],
}

/// Returned when a property edit should be committed as an undo command.
pub struct PropertyEditCommit {
    pub face: (usize, usize, usize),
    pub old_positions: [Vec3; 4],
    pub old_uvs: [Vec2; 4],
    pub old_colors: [Vec4; 4],
    pub new_positions: [Vec3; 4],
    pub new_uvs: [Vec2; 4],
    pub new_colors: [Vec4; 4],
}

/// Draw the properties panel (right side, below layers).
/// Returns a PropertyEditCommit when a deferred edit should be finalized.
pub fn draw_properties_panel(
    ui: &mut egui::Ui,
    scene: &mut Scene,
    edit_state: &EditState,
    snapshot: &mut Option<PropertyEditSnapshot>,
) -> Option<PropertyEditCommit> {
    let sel = &edit_state.selection;
    let mut commit = None;

    if sel.is_empty() {
        // If there's a pending snapshot and selection was cleared, commit it
        if let Some(snap) = snapshot.take()
            && let Some(face) = scene.layers.get(snap.face.0)
                .and_then(|l| l.objects.get(snap.face.1))
                .and_then(|o| o.faces.get(snap.face.2))
            && (snap.positions != face.positions || snap.uvs != face.uvs || snap.colors != face.colors)
        {
            commit = Some(PropertyEditCommit {
                face: snap.face,
                old_positions: snap.positions,
                old_uvs: snap.uvs,
                old_colors: snap.colors,
                new_positions: face.positions,
                new_uvs: face.uvs,
                new_colors: face.colors,
            });
        }
        ui.label("No selection");
        return commit;
    }

    // Show face properties
    if !sel.faces.is_empty() {
        ui.label(format!("{} face(s) selected", sel.faces.len()));

        if sel.faces.len() == 1 {
            let (li, oi, fi) = sel.faces[0];
            let current_face = (li, oi, fi);

            // Check if the edited face changed — if so, commit the old snapshot
            if let &mut Some(ref snap) = snapshot
                && snap.face != current_face
            {
                let old_snap = snapshot.take().unwrap();
                if let Some(face) = scene.layers.get(old_snap.face.0)
                    .and_then(|l| l.objects.get(old_snap.face.1))
                    .and_then(|o| o.faces.get(old_snap.face.2))
                    && (old_snap.positions != face.positions || old_snap.uvs != face.uvs || old_snap.colors != face.colors)
                {
                    commit = Some(PropertyEditCommit {
                        face: old_snap.face,
                        old_positions: old_snap.positions,
                        old_uvs: old_snap.uvs,
                        old_colors: old_snap.colors,
                        new_positions: face.positions,
                        new_uvs: face.uvs,
                        new_colors: face.colors,
                    });
                }
            }

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
                        if snapshot.is_none() {
                            // Already mutated, but we need the OLD value. We'll take it from commit below.
                        }
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

                // Take snapshot if we don't have one yet for this face
                if snapshot.is_none() {
                    *snapshot = Some(PropertyEditSnapshot {
                        face: current_face,
                        positions: face.positions,
                        uvs: face.uvs,
                        colors: face.colors,
                    });
                }
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

    commit
}
