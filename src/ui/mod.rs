mod tools_panel;
pub mod properties_panel;
mod layers_panel;
pub mod tileset_panel;
pub mod uv_panel;
pub mod paint_panel;
pub mod viewcube;
pub mod rulers;

use crate::scene::{Scene, Layer};
use crate::tools::ToolMode;
use crate::tools::draw::DrawState;
use crate::tools::edit::EditState;
use crate::history::History;
use properties_panel::PropertyEditSnapshot;

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
    // UV operations
    UVRotateCW,
    UVRotateCCW,
    UVFlipH,
    UVFlipV,
    // Geometry operations
    MergeVertices,
    MirrorX,
    MirrorY,
    MirrorZ,
    // Edge operations
    SplitEdge,
    CollapseEdge,
    // Import
    ImportObj,
    ImportGlb,
    ImportGltf,
    ImportDae,
    // Export (additional formats)
    ExportGltf,
    ExportDae,
    // Camera bookmarks
    SaveBookmark(usize),
    RecallBookmark(usize),
    // Lighting
    ToggleLighting,
    // Advanced selection
    SelectByNormal,
    SelectOverlapping,
    SelectByTilebrush,
    SelectEdgeLoop,
    SelectFacesFromVertices,
    // Tileset management
    RemoveTileset(usize),
    DuplicateTileset(usize),
    ReplaceTileset(usize),
    ExportTileset(usize),
    RemoveUnusedTilesets,
    // Paint editor
    PaintSyncToGpu,
    OpenPaintEditor,
    // Material settings
    RebuildMaterial(usize),
    // Prefab operations
    CreatePrefab,
    DeconstructPrefab,
    DeletePrefab(usize),
    RenamePrefab(usize, String),
    // Bone operations
    AddBone,
    DeleteBone(usize),
    // Skybox
    ToggleSkybox,
    LoadSkyboxImage,
    SetSkyboxGradient,
    // Screenshot
    TakeScreenshot,
    // ViewCube camera navigation
    ViewCubeClick(viewcube::ViewCubeClick),
    // Keybindings
    OpenKeybindingsEditor,
    ResetKeybindings,
    // Settings
    OpenSettings,
    ResetSettings,
    // Backface culling toggle
    ToggleBackfaceCulling,
    // Triangle operations
    TriangleDivide(u8), // diagonal: 0 = 0→2, 1 = 1→3
    TriangleMerge,
    SelectTriangles,
    // Vertex alignment operations
    PushVertices,
    PullVertices,
    CenterToX,
    CenterToY,
    CenterToZ,
    StraightenVertices,
    // Recent files
    OpenRecentFile(usize),
    // UV vertex drag from UV panel
    UvVertexDrag {
        /// (index into selection.faces, vertex 0-3)
        targets: Vec<(usize, usize)>,
        /// Original UV positions before drag
        original_uvs: Vec<glam::Vec2>,
        /// Delta applied to each target
        delta: glam::Vec2,
    },
}

/// Editable light settings passed to draw_ui.
pub struct LightSettings {
    pub enabled: bool,
    pub direction: [f32; 3],
    pub color: [f32; 3],
    pub intensity: f32,
    pub ambient: [f32; 3],
}

/// Editable skybox settings passed to draw_ui.
pub struct SkyboxSettings {
    pub enabled: bool,
    pub top_color: [f32; 4],
    pub bottom_color: [f32; 4],
    pub has_texture: bool,
    pub use_texture: bool,
}

/// Result from draw_ui, including optional property edit commit.
pub struct UiResult {
    pub action: UiAction,
    pub property_commit: Option<properties_panel::PropertyEditCommit>,
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
    property_snapshot: &mut Option<PropertyEditSnapshot>,
    recent_files: &[std::path::PathBuf],
    light: &mut LightSettings,
    skybox: &mut SkyboxSettings,
    uv_state: &mut uv_panel::UvPanelState,
    paint_state: &mut crate::paint::PaintState,
    screenshot_msg: Option<&str>,
    camera_yaw: f32,
    camera_pitch: f32,
    keybindings: &mut crate::keybindings::Keybindings,
    keybindings_editor_open: &mut bool,
    settings: &mut crate::settings::Settings,
    settings_open: &mut bool,
    settings_tab: &mut crate::settings::SettingsTab,
    backface_culling: bool,
    rulers_visible: &mut bool,
    view_proj: glam::Mat4,
    screen_size: glam::Vec2,
    grid_size: f32,
    crosshair_y: f32,
) -> UiResult {
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
                // Recent files
                if !recent_files.is_empty() {
                    ui.menu_button("Recent Files", |ui| {
                        for (i, path) in recent_files.iter().enumerate() {
                            let name = path.file_name()
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_else(|| path.to_string_lossy().to_string());
                            if ui.button(&name).on_hover_text(path.to_string_lossy().to_string()).clicked() {
                                action = UiAction::OpenRecentFile(i);
                                ui.close();
                            }
                        }
                    });
                    ui.separator();
                }
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
                    if ui.button("glTF JSON (.gltf)").clicked() {
                        action = UiAction::ExportGltf;
                        ui.close();
                    }
                    if ui.button("Collada (.dae)").clicked() {
                        action = UiAction::ExportDae;
                        ui.close();
                    }
                });
                ui.menu_button("Import", |ui| {
                    if ui.button("Wavefront OBJ (.obj)").clicked() {
                        action = UiAction::ImportObj;
                        ui.close();
                    }
                    if ui.button("glTF Binary (.glb)").clicked() {
                        action = UiAction::ImportGlb;
                        ui.close();
                    }
                    if ui.button("glTF JSON (.gltf)").clicked() {
                        action = UiAction::ImportGltf;
                        ui.close();
                    }
                    if ui.button("Collada (.dae)").clicked() {
                        action = UiAction::ImportDae;
                        ui.close();
                    }
                });
                ui.separator();
                if ui.button("Screenshot  F12").clicked() {
                    action = UiAction::TakeScreenshot;
                    ui.close();
                }
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
                ui.separator();
                ui.menu_button("Select...", |ui| {
                    if ui.button("By Normal (facing camera)").clicked() {
                        action = UiAction::SelectByNormal;
                        ui.close();
                    }
                    if ui.button("Overlapping Faces").clicked() {
                        action = UiAction::SelectOverlapping;
                        ui.close();
                    }
                    if ui.button("By Tilebrush UVs").clicked() {
                        action = UiAction::SelectByTilebrush;
                        ui.close();
                    }
                    if ui.button("Edge Loop").on_hover_text("Extend selection along edge loop").clicked() {
                        action = UiAction::SelectEdgeLoop;
                        ui.close();
                    }
                    if ui.button("Faces from Vertices").on_hover_text("Select faces touching selected vertices").clicked() {
                        action = UiAction::SelectFacesFromVertices;
                        ui.close();
                    }
                });
                ui.separator();
                if ui.button("Keybindings...").clicked() {
                    action = UiAction::OpenKeybindingsEditor;
                    ui.close();
                }
                if ui.button("Preferences...").clicked() {
                    action = UiAction::OpenSettings;
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
                let cull_label = if backface_culling { "Backface Culling [ON]" } else { "Backface Culling" };
                if ui.button(cull_label).clicked() {
                    action = UiAction::ToggleBackfaceCulling;
                    ui.close();
                }
                let ruler_label = if *rulers_visible { "Rulers [ON]" } else { "Rulers" };
                if ui.button(ruler_label).clicked() {
                    *rulers_visible = !*rulers_visible;
                    ui.close();
                }
                let light_label = if light.enabled { "Lighting [ON]" } else { "Lighting" };
                if ui.button(light_label).clicked() {
                    action = UiAction::ToggleLighting;
                    ui.close();
                }
                if light.enabled {
                    ui.indent("light_settings", |ui| {
                        ui.horizontal(|ui| {
                            ui.label("Dir X:");
                            ui.add(egui::DragValue::new(&mut light.direction[0]).range(-1.0..=1.0).speed(0.01));
                            ui.label("Y:");
                            ui.add(egui::DragValue::new(&mut light.direction[1]).range(-1.0..=1.0).speed(0.01));
                            ui.label("Z:");
                            ui.add(egui::DragValue::new(&mut light.direction[2]).range(-1.0..=1.0).speed(0.01));
                        });
                        ui.horizontal(|ui| {
                            ui.label("Intensity:");
                            ui.add(egui::DragValue::new(&mut light.intensity).range(0.0..=2.0).speed(0.01));
                        });
                        ui.horizontal(|ui| {
                            ui.label("Color:");
                            ui.color_edit_button_rgb(&mut light.color);
                        });
                        ui.horizontal(|ui| {
                            ui.label("Ambient:");
                            ui.color_edit_button_rgb(&mut light.ambient);
                        });
                    });
                }
                ui.separator();
                ui.menu_button("Bookmarks", |ui| {
                    for i in 0..5 {
                        if ui.button(format!("Save Bookmark {}  Ctrl+Shift+{}", i + 1, i + 1)).clicked() {
                            action = UiAction::SaveBookmark(i);
                            ui.close();
                        }
                    }
                    ui.separator();
                    for i in 0..5 {
                        if ui.button(format!("Recall Bookmark {}  Ctrl+{}", i + 1, i + 1)).clicked() {
                            action = UiAction::RecallBookmark(i);
                            ui.close();
                        }
                    }
                });
                ui.separator();
                let float_label = if draw_state.tileset_panel_floating {
                    "Dock Tileset Panel  Ctrl+Shift+T"
                } else {
                    "Float Tileset Panel  Ctrl+Shift+T"
                };
                if ui.button(float_label).clicked() {
                    draw_state.tileset_panel_floating = !draw_state.tileset_panel_floating;
                    ui.close();
                }
                let uv_label = if uv_state.open { "UV Editor [ON]  Ctrl+U" } else { "UV Editor  Ctrl+U" };
                if ui.button(uv_label).clicked() {
                    uv_state.open = !uv_state.open;
                    ui.close();
                }
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label("Background:");
                    ui.color_edit_button_rgb(bg_color);
                });
                ui.separator();
                let sky_label = if skybox.enabled { "Skybox [ON]" } else { "Skybox" };
                if ui.button(sky_label).clicked() {
                    action = UiAction::ToggleSkybox;
                    ui.close();
                }
                if skybox.enabled {
                    ui.indent("skybox_settings", |ui| {
                        ui.horizontal(|ui| {
                            ui.label("Top:");
                            let mut top3 = [skybox.top_color[0], skybox.top_color[1], skybox.top_color[2]];
                            if ui.color_edit_button_rgb(&mut top3).changed() {
                                skybox.top_color = [top3[0], top3[1], top3[2], 1.0];
                            }
                        });
                        ui.horizontal(|ui| {
                            ui.label("Bottom:");
                            let mut bot3 = [skybox.bottom_color[0], skybox.bottom_color[1], skybox.bottom_color[2]];
                            if ui.color_edit_button_rgb(&mut bot3).changed() {
                                skybox.bottom_color = [bot3[0], bot3[1], bot3[2], 1.0];
                            }
                        });
                        if skybox.has_texture
                            && ui.checkbox(&mut skybox.use_texture, "Use Texture").changed()
                        {
                            action = UiAction::SetSkyboxGradient;
                        }
                        if ui.button("Load Panorama...").clicked() {
                            action = UiAction::LoadSkyboxImage;
                            ui.close();
                        }
                    });
                }
            });
        });
    });

    // Tools panel (left)
    let tools_action = tools_panel::draw_tools_panel(ctx, tool_mode, draw_state, edit_state, scene);
    if !matches!(tools_action, UiAction::None) {
        action = tools_action;
    }

    // Layers + Properties panel (right)
    let (layer_action, prop_commit) = layers_panel::draw_layers_panel(ctx, scene, edit_state, property_snapshot);
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
            tileset_panel::TilesetAction::RemoveTileset(idx) => {
                action = UiAction::RemoveTileset(idx);
            }
            tileset_panel::TilesetAction::DuplicateTileset(idx) => {
                action = UiAction::DuplicateTileset(idx);
            }
            tileset_panel::TilesetAction::ReplaceTileset(idx) => {
                action = UiAction::ReplaceTileset(idx);
            }
            tileset_panel::TilesetAction::ExportTileset(idx) => {
                action = UiAction::ExportTileset(idx);
            }
            tileset_panel::TilesetAction::RemoveUnusedTilesets => {
                action = UiAction::RemoveUnusedTilesets;
            }
            tileset_panel::TilesetAction::OpenPaintEditor => {
                action = UiAction::OpenPaintEditor;
            }
            tileset_panel::TilesetAction::RebuildMaterial(idx) => {
                action = UiAction::RebuildMaterial(idx);
            }
            tileset_panel::TilesetAction::None => {}
        }
    }

    // UV Editor panel (floating window)
    {
        let uv_action = uv_panel::draw_uv_panel(ctx, scene, edit_state, uv_state);
        if !matches!(uv_action, UiAction::None) {
            action = uv_action;
        }
    }

    // Paint Editor panel (floating window)
    {
        let paint_action = paint_panel::draw_paint_panel(ctx, paint_state);
        if matches!(paint_action, paint_panel::PaintAction::SyncToGpu) {
            action = UiAction::PaintSyncToGpu;
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
            let sel_count = sel.faces.len() + sel.objects.len() + sel.vertices.len() + sel.edges.len();
            if sel_count > 0 {
                ui.label(format!("Selected: {sel_count}"));
                ui.separator();
            }
            if wireframe {
                ui.label("Wireframe");
                ui.separator();
            }
            if backface_culling {
                ui.label("Culling");
                ui.separator();
            }
            if light.enabled {
                ui.label("Lit");
                ui.separator();
            }
            if let Some(msg) = screenshot_msg {
                ui.label(egui::RichText::new(msg).color(egui::Color32::from_rgb(100, 255, 100)));
            }
        });
    });

    // Keybindings editor window
    if *keybindings_editor_open {
        let mut open = true;
        egui::Window::new("Keybindings")
            .open(&mut open)
            .resizable(true)
            .default_size([400.0, 500.0])
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if ui.button("Reset to Defaults").clicked() {
                        action = UiAction::ResetKeybindings;
                    }
                    if ui.button("Save").clicked() {
                        keybindings.save();
                    }
                });
                ui.separator();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    egui::Grid::new("keybindings_grid")
                        .num_columns(2)
                        .striped(true)
                        .min_col_width(180.0)
                        .show(ui, |ui| {
                            ui.strong("Action");
                            ui.strong("Key");
                            ui.end_row();
                            for &(act, name) in crate::keybindings::ALL_ACTIONS {
                                ui.label(name);
                                let display = keybindings.display(act);
                                ui.label(&display);
                                ui.end_row();
                            }
                        });
                });
            });
        if !open {
            *keybindings_editor_open = false;
        }
    }

    // Settings dialog
    if *settings_open {
        use crate::settings::SettingsTab;
        let mut open = true;
        egui::Window::new("Preferences")
            .open(&mut open)
            .resizable(true)
            .default_size([450.0, 400.0])
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.selectable_value(settings_tab, SettingsTab::Camera, "Camera");
                    ui.selectable_value(settings_tab, SettingsTab::Display, "Display");
                    ui.selectable_value(settings_tab, SettingsTab::Draw, "Draw");
                    ui.selectable_value(settings_tab, SettingsTab::Edit, "Edit");
                });
                ui.separator();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    match settings_tab {
                        SettingsTab::Camera => {
                            let c = &mut settings.camera;
                            ui.horizontal(|ui| {
                                ui.label("FOV (degrees):");
                                ui.add(egui::DragValue::new(&mut c.fov_degrees).range(10.0..=120.0).speed(0.5));
                            });
                            ui.horizontal(|ui| {
                                ui.label("Near plane:");
                                ui.add(egui::DragValue::new(&mut c.near_plane).range(0.01..=10.0).speed(0.01));
                            });
                            ui.horizontal(|ui| {
                                ui.label("Far plane:");
                                ui.add(egui::DragValue::new(&mut c.far_plane).range(100.0..=100000.0).speed(10.0));
                            });
                            ui.horizontal(|ui| {
                                ui.label("Orbit sensitivity:");
                                ui.add(egui::DragValue::new(&mut c.orbit_sensitivity).range(0.001..=0.05).speed(0.0005));
                            });
                            ui.horizontal(|ui| {
                                ui.label("Pan sensitivity:");
                                ui.add(egui::DragValue::new(&mut c.pan_sensitivity).range(0.001..=0.1).speed(0.001));
                            });
                            ui.horizontal(|ui| {
                                ui.label("Freelook sensitivity:");
                                ui.add(egui::DragValue::new(&mut c.freelook_sensitivity).range(0.001..=0.05).speed(0.0005));
                            });
                            ui.horizontal(|ui| {
                                ui.label("Freelook speed:");
                                ui.add(egui::DragValue::new(&mut c.freelook_speed).range(0.01..=1.0).speed(0.005));
                            });
                            ui.horizontal(|ui| {
                                ui.label("Zoom speed:");
                                ui.add(egui::DragValue::new(&mut c.zoom_speed).range(0.1..=5.0).speed(0.05));
                            });
                            ui.checkbox(&mut c.invert_orbit_y, "Invert orbit Y axis");
                        }
                        SettingsTab::Display => {
                            let d = &mut settings.display;
                            ui.horizontal(|ui| {
                                ui.label("Background:");
                                ui.color_edit_button_rgb(&mut d.bg_color);
                            });
                            ui.horizontal(|ui| {
                                ui.label("Grid:");
                                let mut c3 = [d.grid_color[0], d.grid_color[1], d.grid_color[2]];
                                if ui.color_edit_button_rgb(&mut c3).changed() {
                                    d.grid_color = [c3[0], c3[1], c3[2], d.grid_color[3]];
                                }
                            });
                            ui.horizontal(|ui| {
                                ui.label("Wireframe:");
                                let mut c3 = [d.wireframe_color[0], d.wireframe_color[1], d.wireframe_color[2]];
                                if ui.color_edit_button_rgb(&mut c3).changed() {
                                    d.wireframe_color = [c3[0], c3[1], c3[2], d.wireframe_color[3]];
                                }
                            });
                            ui.horizontal(|ui| {
                                ui.label("Selection:");
                                let mut c3 = [d.selection_color[0], d.selection_color[1], d.selection_color[2]];
                                if ui.color_edit_button_rgb(&mut c3).changed() {
                                    d.selection_color = [c3[0], c3[1], c3[2], d.selection_color[3]];
                                }
                            });
                            ui.horizontal(|ui| {
                                ui.label("Vertex:");
                                let mut c3 = [d.vertex_color[0], d.vertex_color[1], d.vertex_color[2]];
                                if ui.color_edit_button_rgb(&mut c3).changed() {
                                    d.vertex_color = [c3[0], c3[1], c3[2], d.vertex_color[3]];
                                }
                            });
                            ui.horizontal(|ui| {
                                ui.label("Edge:");
                                let mut c3 = [d.edge_color[0], d.edge_color[1], d.edge_color[2]];
                                if ui.color_edit_button_rgb(&mut c3).changed() {
                                    d.edge_color = [c3[0], c3[1], c3[2], d.edge_color[3]];
                                }
                            });
                            ui.horizontal(|ui| {
                                ui.label("Hover:");
                                let mut c3 = [d.hover_color[0], d.hover_color[1], d.hover_color[2]];
                                if ui.color_edit_button_rgb(&mut c3).changed() {
                                    d.hover_color = [c3[0], c3[1], c3[2], d.hover_color[3]];
                                }
                            });
                            ui.horizontal(|ui| {
                                ui.label("Preview:");
                                let mut c3 = [d.preview_color[0], d.preview_color[1], d.preview_color[2]];
                                if ui.color_edit_button_rgb(&mut c3).changed() {
                                    d.preview_color = [c3[0], c3[1], c3[2], d.preview_color[3]];
                                }
                            });
                            ui.horizontal(|ui| {
                                ui.label("Vertex size:");
                                ui.add(egui::DragValue::new(&mut d.vertex_size).range(0.05..=1.0).speed(0.01));
                            });
                            ui.horizontal(|ui| {
                                ui.label("Undo limit:");
                                let mut val = d.undo_limit as f64;
                                if ui.add(egui::DragValue::new(&mut val).range(10.0..=1000.0).speed(1.0)).changed() {
                                    d.undo_limit = val as usize;
                                }
                            });
                        }
                        SettingsTab::Draw => {
                            let dr = &mut settings.draw;
                            ui.horizontal(|ui| {
                                ui.label("Default paint color:");
                                let mut c3 = [dr.default_paint_color[0], dr.default_paint_color[1], dr.default_paint_color[2]];
                                if ui.color_edit_button_rgb(&mut c3).changed() {
                                    dr.default_paint_color = [c3[0], c3[1], c3[2], dr.default_paint_color[3]];
                                }
                            });
                            ui.horizontal(|ui| {
                                ui.label("Default paint opacity:");
                                ui.add(egui::DragValue::new(&mut dr.default_paint_opacity).range(0.0..=1.0).speed(0.01));
                            });
                            ui.horizontal(|ui| {
                                ui.label("Default paint radius:");
                                ui.add(egui::DragValue::new(&mut dr.default_paint_radius).range(0.0..=10.0).speed(0.1));
                            });
                        }
                        SettingsTab::Edit => {
                            let e = &mut settings.edit;
                            ui.horizontal(|ui| {
                                ui.label("Vertex pick threshold (px):");
                                ui.add(egui::DragValue::new(&mut e.vertex_pick_threshold).range(4.0..=50.0).speed(0.5));
                            });
                            ui.horizontal(|ui| {
                                ui.label("Merge distance:");
                                ui.add(egui::DragValue::new(&mut e.merge_distance).range(0.0001..=1.0).speed(0.0001));
                            });
                            ui.checkbox(&mut e.auto_flatten_uvs, "Auto-flatten UVs on vertex edit");
                        }
                    }
                });
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("Reset to Defaults").clicked() {
                        action = UiAction::ResetSettings;
                    }
                    if ui.button("Save").clicked() {
                        settings.save();
                    }
                });
            });
        if !open {
            *settings_open = false;
            settings.save();
        }
    }

    // Rulers overlay
    if *rulers_visible {
        rulers::draw_rulers(ctx, view_proj, screen_size, grid_size, crosshair_y);
    }

    // ViewCube overlay
    if let Some(click) = viewcube::draw_viewcube(ctx, camera_yaw, camera_pitch) {
        action = UiAction::ViewCubeClick(click);
    }

    UiResult {
        action,
        property_commit: prop_commit,
    }
}
