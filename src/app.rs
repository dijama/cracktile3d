use std::sync::Arc;

use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowAttributes, WindowId};

use winit::keyboard::KeyCode;

use crate::render::Renderer;
use crate::render::camera::{CameraBookmark, CameraMode};
use crate::render::gizmo::{self, GizmoAxis};
use crate::input::InputState;
use crate::scene::mesh::Face;
use crate::scene::{Scene, GRID_PRESETS};
use crate::tools::ToolMode;
use crate::tools::draw::{DrawState, DrawTool, camera_placement_normal};
use crate::tools::edit::{EditState, GizmoMode};
use crate::history::History;
use crate::history::commands;
use crate::ui::{UiAction, UiResult};
use crate::keybindings::Keybindings;
use crate::ui::properties_panel::PropertyEditSnapshot;
use crate::ui::uv_panel::UvPanelState;
use crate::paint::PaintState;
use crate::util::picking::{self, Ray};

/// Pending tileset load awaiting tile-size confirmation.
struct PendingTilesetLoad {
    path: std::path::PathBuf,
    tile_width: u32,
    tile_height: u32,
}

/// Pending confirmation dialog.
enum ConfirmDialog {
    NewScene,
    Quit,
}

/// Data stored in the clipboard for copy/paste.
struct ClipboardData {
    faces: Vec<Face>,
    centroid: glam::Vec3,
    tileset_index: Option<usize>,
}

/// Top-level application state.
pub struct App {
    gpu: Option<GpuState>,
    scene: Scene,
    input: InputState,
    tool_mode: ToolMode,
    draw_state: DrawState,
    edit_state: EditState,
    history: History,
    pending_action: Option<UiAction>,
    pending_tileset: Option<PendingTilesetLoad>,
    wireframe: bool,
    clipboard: Option<ClipboardData>,
    bg_color: [f32; 3],
    /// Last save path for quick-save (Ctrl+S without dialog after first save)
    last_save_path: Option<std::path::PathBuf>,
    /// Preview faces for tile placement ghost (computed each frame in Draw mode)
    preview_faces: Vec<Face>,
    /// Face currently hovered in Edit mode (for highlight-on-hover)
    hover_face: Option<(usize, usize, usize)>,
    /// Tracks unsaved changes for title bar indicator and confirm dialogs
    has_unsaved_changes: bool,
    /// Pending confirmation dialog (e.g., "New Scene" when unsaved)
    confirm_dialog: Option<ConfirmDialog>,
    /// Deferred property edit snapshot for undo
    property_snapshot: Option<PropertyEditSnapshot>,
    /// Recent files list (max 10)
    recent_files: Vec<std::path::PathBuf>,
    /// Camera bookmarks (up to 5)
    camera_bookmarks: [Option<CameraBookmark>; 5],
    /// Lighting preview enabled
    lighting_enabled: bool,
    /// Last position where a tile was placed during drag-painting (to avoid duplicates).
    last_placed_pos: Option<glam::Vec3>,
    /// Pre-built gizmo line vertices for this frame.
    gizmo_lines: Vec<crate::render::vertex::LineVertex>,
    /// UV editor panel state.
    uv_state: UvPanelState,
    /// Paint editor state.
    paint_state: PaintState,
    /// User-configurable keybindings.
    keybindings: Keybindings,
    /// Whether the keybindings editor is open.
    keybindings_editor_open: bool,
    /// User settings/preferences.
    settings: crate::settings::Settings,
    /// Whether the settings dialog is open.
    settings_open: bool,
    /// Active tab in settings dialog.
    settings_tab: crate::settings::SettingsTab,
    /// Whether rulers are visible.
    rulers_visible: bool,
    /// Rectangle fill drag start position (grid-snapped, on placement plane).
    rect_fill_start: Option<(glam::Vec3, glam::Vec3)>, // (center, normal)
    /// Set to true to capture a screenshot at the end of this frame.
    screenshot_pending: bool,
    /// Countdown timer for screenshot status bar indicator (seconds remaining).
    screenshot_flash: f32,
    /// Path of last screenshot for status bar display.
    screenshot_last_path: Option<String>,
}

/// Everything that requires the window to exist.
struct GpuState {
    window: Arc<Window>,
    renderer: Renderer,
    egui_state: egui_winit::State,
    egui_renderer: egui_wgpu::Renderer,
}

impl App {
    pub fn new(_event_loop: &winit::event_loop::EventLoop<()>) -> Self {
        let recent_files = crate::io::load_recent_files();
        Self {
            gpu: None,
            scene: Scene::new(),
            input: InputState::new(),
            tool_mode: ToolMode::Draw,
            draw_state: DrawState::new(),
            edit_state: EditState::new(),
            history: History::new(),
            pending_action: None,
            pending_tileset: None,
            wireframe: false,
            clipboard: None,
            bg_color: crate::settings::Settings::load().display.bg_color,
            last_save_path: None,
            preview_faces: Vec::new(),
            hover_face: None,
            has_unsaved_changes: false,
            confirm_dialog: None,
            property_snapshot: None,
            recent_files,
            camera_bookmarks: [None, None, None, None, None],
            lighting_enabled: false,
            last_placed_pos: None,
            gizmo_lines: Vec::new(),
            uv_state: UvPanelState::new(),
            paint_state: PaintState::new(),
            keybindings: Keybindings::load(),
            keybindings_editor_open: false,
            settings: crate::settings::Settings::load(),
            settings_open: false,
            settings_tab: crate::settings::SettingsTab::Camera,
            rulers_visible: false,
            rect_fill_start: None,
            screenshot_pending: false,
            screenshot_flash: 0.0,
            screenshot_last_path: None,
        }
    }

}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.gpu.is_some() {
            return;
        }

        let attrs = WindowAttributes::default()
            .with_title("Cracktile 3D")
            .with_inner_size(winit::dpi::LogicalSize::new(1280u32, 720u32));
        let window = Arc::new(event_loop.create_window(attrs).expect("failed to create window"));

        let renderer = pollster::block_on(Renderer::new(window.clone()));

        let egui_ctx = egui::Context::default();
        let egui_state = egui_winit::State::new(
            egui_ctx,
            egui::ViewportId::ROOT,
            &window,
            Some(window.scale_factor() as f32),
            None,
            None,
        );
        let egui_renderer = egui_wgpu::Renderer::new(
            &renderer.device,
            renderer.surface_format,
            None,
            1,
            false,
        );

        self.gpu = Some(GpuState {
            window,
            renderer,
            egui_state,
            egui_renderer,
        });
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(gpu) = &mut self.gpu else { return };

        // Let egui process the event first
        let egui_response = gpu.egui_state.on_window_event(&gpu.window, &event);
        let egui_consumed = egui_response.consumed;

        match &event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::Resized(new_size) => {
                gpu.renderer.resize(new_size.width, new_size.height);
                gpu.window.request_redraw();
            }
            WindowEvent::RedrawRequested => {
                self.redraw();
            }
            _ => {}
        }

        // Forward input events to our input system if egui didn't consume them
        if !egui_consumed {
            self.input.handle_event(&event);
        }

        // Always request redraw to keep the render loop going
        if let Some(gpu) = &self.gpu {
            gpu.window.request_redraw();
        }
    }
}

impl App {
    fn process_input(&mut self) {
        let Some(gpu) = &mut self.gpu else { return };

        // Freelook camera: activate on right-click hold in Edit mode (when not Space)
        let in_freelook = gpu.renderer.camera.mode == CameraMode::Freelook;
        if self.tool_mode == ToolMode::Edit && !self.input.space_held()
            && self.input.right_pressed && !in_freelook
        {
            gpu.renderer.camera.enter_freelook();
        }
        if in_freelook && !self.input.right_pressed {
            gpu.renderer.camera.exit_freelook();
        }

        // Apply camera settings from preferences
        let cam_settings = &self.settings.camera;
        gpu.renderer.camera.fov_y = cam_settings.fov_degrees.to_radians();
        gpu.renderer.camera.near = cam_settings.near_plane;
        gpu.renderer.camera.far = cam_settings.far_plane;

        if gpu.renderer.camera.mode == CameraMode::Freelook {
            // Freelook mouse look
            gpu.renderer.camera.freelook_look(
                -self.input.mouse_delta.x * cam_settings.freelook_sensitivity,
                self.input.mouse_delta.y * cam_settings.freelook_sensitivity,
            );

            // Freelook WASD movement
            let mut forward = 0.0_f32;
            let mut right = 0.0_f32;
            let mut up = 0.0_f32;
            if self.input.key_held(KeyCode::KeyW) { forward += 1.0; }
            if self.input.key_held(KeyCode::KeyS) { forward -= 1.0; }
            if self.input.key_held(KeyCode::KeyD) { right += 1.0; }
            if self.input.key_held(KeyCode::KeyA) { right -= 1.0; }
            if self.input.key_held(KeyCode::KeyE) { up += 1.0; }
            if self.input.key_held(KeyCode::KeyQ) { up -= 1.0; }
            if forward != 0.0 || right != 0.0 || up != 0.0 {
                gpu.renderer.camera.freelook_move(forward, right, up);
            }

            // Scroll adjusts freelook speed
            if self.input.scroll_delta != 0.0 {
                gpu.renderer.camera.freelook_speed = (gpu.renderer.camera.freelook_speed + self.input.scroll_delta * 0.02).max(0.01);
            }
        } else {
            // Camera orbit (Space + left drag, or middle mouse drag)
            let orbiting = (self.input.space_held() && self.input.left_pressed)
                || (self.input.middle_pressed && !self.input.key_held(KeyCode::ShiftLeft) && !self.input.key_held(KeyCode::ShiftRight));
            let invert_y = if cam_settings.invert_orbit_y { 1.0 } else { -1.0 };
            if orbiting {
                gpu.renderer.camera.orbit(
                    -self.input.mouse_delta.x * cam_settings.orbit_sensitivity,
                    invert_y * self.input.mouse_delta.y * cam_settings.orbit_sensitivity,
                );
            }

            // Camera pan (Space + right drag, or Shift + middle mouse drag)
            let panning = (self.input.space_held() && self.input.right_pressed)
                || (self.input.middle_pressed && (self.input.key_held(KeyCode::ShiftLeft) || self.input.key_held(KeyCode::ShiftRight)));
            if panning {
                let pan_sens = cam_settings.pan_sensitivity * gpu.renderer.camera.distance;
                gpu.renderer.camera.pan(
                    -self.input.mouse_delta.x * pan_sens,
                    self.input.mouse_delta.y * pan_sens,
                );
            }

            // Camera zoom (scroll wheel)
            if self.input.scroll_delta != 0.0 {
                gpu.renderer.camera.zoom(self.input.scroll_delta * cam_settings.zoom_speed);
            }
        }

        // Toggle projection (Numpad 5)
        if self.input.key_just_pressed(KeyCode::Numpad5) {
            gpu.renderer.camera.toggle_projection();
        }

        // Numpad preset views
        let ctrl = self.input.key_held(KeyCode::ControlLeft) || self.input.key_held(KeyCode::ControlRight);
        if self.input.key_just_pressed(KeyCode::Numpad1) {
            if ctrl { gpu.renderer.camera.set_view_back(); } else { gpu.renderer.camera.set_view_front(); }
        }
        if self.input.key_just_pressed(KeyCode::Numpad3) {
            if ctrl { gpu.renderer.camera.set_view_left(); } else { gpu.renderer.camera.set_view_right(); }
        }
        if self.input.key_just_pressed(KeyCode::Numpad7) {
            if ctrl { gpu.renderer.camera.set_view_bottom(); } else { gpu.renderer.camera.set_view_top(); }
        }

        // Numpad orbit by 15-degree increments
        let orbit_step = 15.0_f32.to_radians();
        if self.input.key_just_pressed(KeyCode::Numpad4) {
            gpu.renderer.camera.orbit(-orbit_step, 0.0);
        }
        if self.input.key_just_pressed(KeyCode::Numpad6) {
            gpu.renderer.camera.orbit(orbit_step, 0.0);
        }
        if self.input.key_just_pressed(KeyCode::Numpad8) {
            gpu.renderer.camera.orbit(0.0, orbit_step);
        }
        if self.input.key_just_pressed(KeyCode::Numpad2) {
            gpu.renderer.camera.orbit(0.0, -orbit_step);
        }

        // WASD crosshair movement (only in Draw mode, when Space is NOT held)
        if self.tool_mode == ToolMode::Draw && !self.input.space_held() {
            let step = self.scene.grid_cell_size;
            if self.input.key_just_pressed(KeyCode::KeyW) {
                self.scene.crosshair_pos.z -= step;
            }
            if self.input.key_just_pressed(KeyCode::KeyS) {
                self.scene.crosshair_pos.z += step;
            }
            if self.input.key_just_pressed(KeyCode::KeyA) {
                self.scene.crosshair_pos.x -= step;
            }
            if self.input.key_just_pressed(KeyCode::KeyD) {
                self.scene.crosshair_pos.x += step;
            }
            if self.input.key_just_pressed(KeyCode::KeyQ) {
                self.scene.crosshair_pos.y -= step;
            }
            if self.input.key_just_pressed(KeyCode::KeyE) {
                self.scene.crosshair_pos.y += step;
            }
        }

        // Update placement normal from camera direction
        {
            let cam_fwd = (gpu.renderer.camera.target - gpu.renderer.camera.position).normalize();
            self.draw_state.placement_normal = camera_placement_normal(cam_fwd);
        }

        // Grid preset cycling
        if self.keybindings.is_triggered(crate::keybindings::Action::GridIncrease, &self.input)
            && self.scene.grid_preset_index + 1 < GRID_PRESETS.len()
        {
            self.scene.grid_preset_index += 1;
            self.scene.grid_cell_size = GRID_PRESETS[self.scene.grid_preset_index];
        }
        if self.keybindings.is_triggered(crate::keybindings::Action::GridDecrease, &self.input)
            && self.scene.grid_preset_index > 0
        {
            self.scene.grid_preset_index -= 1;
            self.scene.grid_cell_size = GRID_PRESETS[self.scene.grid_preset_index];
        }

        // Wireframe toggle
        if self.keybindings.is_triggered(crate::keybindings::Action::ToggleWireframe, &self.input) {
            self.wireframe = !self.wireframe;
        }

        // Mode toggle
        if self.keybindings.is_triggered(crate::keybindings::Action::ToggleMode, &self.input) {
            self.tool_mode = match self.tool_mode {
                ToolMode::Draw => ToolMode::Edit,
                ToolMode::Edit => ToolMode::Draw,
            };
        }

        // Create Instance keybinding (Ctrl+Shift+I)
        if self.keybindings.is_triggered(crate::keybindings::Action::CreateInstance, &self.input) {
            self.pending_action = Some(UiAction::CreateInstance);
        }

        // Number keys switch draw tools
        if self.tool_mode == ToolMode::Draw && !self.input.space_held() {
            if self.keybindings.is_triggered(crate::keybindings::Action::ToolTile, &self.input) { self.draw_state.tool = DrawTool::Tile; }
            if self.keybindings.is_triggered(crate::keybindings::Action::ToolSticky, &self.input) { self.draw_state.tool = DrawTool::Sticky; }
            if self.keybindings.is_triggered(crate::keybindings::Action::ToolBlock, &self.input) { self.draw_state.tool = DrawTool::Block; }
            if self.keybindings.is_triggered(crate::keybindings::Action::ToolPrimitive, &self.input) { self.draw_state.tool = DrawTool::Primitive; }
            if self.keybindings.is_triggered(crate::keybindings::Action::ToolVertexColor, &self.input) { self.draw_state.tool = DrawTool::VertexColor; }
            if self.keybindings.is_triggered(crate::keybindings::Action::ToolPrefab, &self.input) { self.draw_state.tool = DrawTool::Prefab; }
        }

        // Draw mode: tilebrush rotation/flip keys
        if self.tool_mode == ToolMode::Draw && !self.input.space_held() {
            if self.keybindings.is_triggered(crate::keybindings::Action::TilebrushRotCW, &self.input) {
                self.draw_state.tilebrush_rotation = (self.draw_state.tilebrush_rotation + 1) % 4;
            }
            if self.keybindings.is_triggered(crate::keybindings::Action::TilebrushRotCCW, &self.input) {
                self.draw_state.tilebrush_rotation = (self.draw_state.tilebrush_rotation + 3) % 4;
            }
            if self.keybindings.is_triggered(crate::keybindings::Action::TilebrushFlipH, &self.input) {
                self.draw_state.tilebrush_flip_h = !self.draw_state.tilebrush_flip_h;
            }
            if self.keybindings.is_triggered(crate::keybindings::Action::TilebrushFlipV, &self.input) {
                self.draw_state.tilebrush_flip_v = !self.draw_state.tilebrush_flip_v;
            }
        }

        // Rectangle fill: Shift+click in Tile tool starts a fill drag
        let shift_held = self.input.key_held(KeyCode::ShiftLeft) || self.input.key_held(KeyCode::ShiftRight);
        if self.tool_mode == ToolMode::Draw
            && self.draw_state.tool == DrawTool::Tile
            && self.input.left_just_clicked
            && shift_held
            && !self.input.space_held()
        {
            let screen_size = glam::Vec2::new(
                gpu.renderer.config.width as f32,
                gpu.renderer.config.height as f32,
            );
            let ray = Ray::from_screen(
                self.input.mouse_pos,
                screen_size,
                gpu.renderer.camera.view_projection(),
            );
            let normal = self.draw_state.placement_normal;
            if let Some(t) = ray.intersect_plane(self.scene.crosshair_pos, normal) {
                let pos = ray.point_at(t);
                let snapped = glam::Vec3::new(
                    (pos.x / self.scene.grid_cell_size).round() * self.scene.grid_cell_size,
                    (pos.y / self.scene.grid_cell_size).round() * self.scene.grid_cell_size,
                    (pos.z / self.scene.grid_cell_size).round() * self.scene.grid_cell_size,
                );
                self.rect_fill_start = Some((snapped, normal));
            }
        }

        // Rectangle fill: on mouse release during fill drag, place all tiles
        if self.rect_fill_start.is_some() {
            if !self.input.left_pressed {
                // Mouse released — place the fill
                if !self.preview_faces.is_empty() {
                    let layer_idx = self.scene.active_layer;
                    let (object_idx, create_object) = crate::tools::draw::find_target_object(&self.scene, layer_idx, self.scene.active_tileset);
                    let cmd = commands::PlaceTile {
                        layer: layer_idx,
                        object: object_idx,
                        faces: self.preview_faces.clone(),
                        create_object,
                        tileset_index: self.scene.active_tileset,
                    };
                    self.history.push(Box::new(cmd), &mut self.scene, &gpu.renderer.device);
                }
                self.rect_fill_start = None;
            } else if !shift_held {
                // Shift released — cancel
                self.rect_fill_start = None;
            }
            // During drag, preview_faces will be computed in the preview section below
            // Skip normal click handling
        } else

        // Draw mode: left click places tile or paints vertex color (when not orbiting with Space)
        if self.tool_mode == ToolMode::Draw
            && self.input.left_just_clicked
            && !self.input.space_held()
        {
            let screen_size = glam::Vec2::new(
                gpu.renderer.config.width as f32,
                gpu.renderer.config.height as f32,
            );
            let ray = Ray::from_screen(
                self.input.mouse_pos,
                screen_size,
                gpu.renderer.camera.view_projection(),
            );

            if self.draw_state.tool == DrawTool::VertexColor {
                // Vertex color tool: paint hit face (with radius/opacity)
                if let Some(hit) = crate::util::picking::pick_face_culled(&ray, &self.scene) {
                    let c = self.draw_state.paint_color;
                    let new_color = glam::Vec4::new(c[0], c[1], c[2], c[3]);
                    let opacity = self.draw_state.paint_opacity;

                    // Find all faces within paint_radius
                    let mut targets = vec![(hit.layer_index, hit.object_index, hit.face_index)];
                    if self.draw_state.paint_radius > 0.0 {
                        let radius_sq = self.draw_state.paint_radius * self.draw_state.paint_radius;
                        for (li, layer) in self.scene.layers.iter().enumerate() {
                            if !layer.visible { continue; }
                            for (oi, obj) in layer.objects.iter().enumerate() {
                                for (fi, face) in obj.faces.iter().enumerate() {
                                    if (li, oi, fi) == (hit.layer_index, hit.object_index, hit.face_index) { continue; }
                                    let center = (face.positions[0] + face.positions[1] + face.positions[2] + face.positions[3]) * 0.25;
                                    if center.distance_squared(hit.position) <= radius_sq {
                                        targets.push((li, oi, fi));
                                    }
                                }
                            }
                        }
                    }

                    // Apply opacity blending
                    let paint_color = if (opacity - 1.0).abs() < f32::EPSILON {
                        new_color
                    } else {
                        // We'll store the blended color; the command captures old/new colors
                        new_color
                    };

                    let cmd = commands::PaintVertexColor {
                        targets,
                        new_color: paint_color,
                        old_colors: Vec::new(),
                    };
                    self.history.push(Box::new(cmd), &mut self.scene, &gpu.renderer.device);
                }
            } else if self.draw_state.tool == DrawTool::Block && self.draw_state.block_subtract {
                // Block subtract mode: compute block AABB and remove overlapping faces
                if let Some(result) = self.draw_state.compute_placement(&self.scene, &ray)
                    && !result.faces.is_empty()
                {
                    let (aabb_min, aabb_max) = compute_faces_aabb(&result.faces);
                    let cmd = commands::SubtractBlock::new(aabb_min, aabb_max);
                    self.history.push(Box::new(cmd), &mut self.scene, &gpu.renderer.device);
                }
            } else {
                let backup = self.draw_state.apply_palette(&mut self.scene);
                if let Some(result) = self.draw_state.compute_placement(&self.scene, &ray) {
                    // Track placement position for drag-painting
                    if self.draw_state.tool == DrawTool::Tile && !result.faces.is_empty() {
                        let center = (result.faces[0].positions[0] + result.faces[0].positions[2]) * 0.5;
                        self.last_placed_pos = Some(center);
                    }
                    let cmd = commands::PlaceTile {
                        layer: result.layer,
                        object: result.object,
                        faces: result.faces,
                        create_object: result.create_object,
                        tileset_index: result.tileset_index,
                    };
                    self.history.push(Box::new(cmd), &mut self.scene, &gpu.renderer.device);
                }
                if let Some(b) = backup {
                    self.draw_state.restore_palette(&mut self.scene, b);
                }
            }
        }

        // Draw mode: drag-painting for Tile tool (continuous placement while dragging)
        if self.tool_mode == ToolMode::Draw
            && self.draw_state.tool == DrawTool::Tile
            && self.input.left_pressed
            && self.input.is_dragging
            && !self.input.left_just_clicked
            && !self.input.space_held()
        {
            let screen_size = glam::Vec2::new(
                gpu.renderer.config.width as f32,
                gpu.renderer.config.height as f32,
            );
            let ray = Ray::from_screen(
                self.input.mouse_pos,
                screen_size,
                gpu.renderer.camera.view_projection(),
            );
            let backup = self.draw_state.apply_palette(&mut self.scene);
            if let Some(result) = self.draw_state.compute_placement(&self.scene, &ray)
                && !result.faces.is_empty()
            {
                let center = (result.faces[0].positions[0] + result.faces[0].positions[2]) * 0.5;
                let should_place = if let Some(last) = self.last_placed_pos {
                    center.distance_squared(last) > 0.001
                } else {
                    true
                };
                if should_place {
                    self.last_placed_pos = Some(center);
                    let cmd = commands::PlaceTile {
                        layer: result.layer,
                        object: result.object,
                        faces: result.faces,
                        create_object: result.create_object,
                        tileset_index: result.tileset_index,
                    };
                    self.history.push(Box::new(cmd), &mut self.scene, &gpu.renderer.device);
                }
            }
            if let Some(b) = backup {
                self.draw_state.restore_palette(&mut self.scene, b);
            }
        }

        // Clear drag-paint tracking when left button released
        if !self.input.left_pressed {
            self.last_placed_pos = None;
        }

        // Draw mode: right click erases tile
        if self.tool_mode == ToolMode::Draw
            && self.input.right_just_clicked
            && !self.input.space_held()
        {
            let screen_size = glam::Vec2::new(
                gpu.renderer.config.width as f32,
                gpu.renderer.config.height as f32,
            );
            let ray = Ray::from_screen(
                self.input.mouse_pos,
                screen_size,
                gpu.renderer.camera.view_projection(),
            );
            if let Some((layer, object, face_index, face)) = self.draw_state.compute_erase(&self.scene, &ray) {
                let cmd = commands::EraseTile { layer, object, face_index, face };
                self.history.push(Box::new(cmd), &mut self.scene, &gpu.renderer.device);
            }
        }

        // Eyedropper: Alt+RMB picks tile UVs from a face
        let alt = self.input.key_held(KeyCode::AltLeft) || self.input.key_held(KeyCode::AltRight);
        if self.input.right_just_clicked && alt && !self.input.space_held() {
            let screen_size = glam::Vec2::new(
                gpu.renderer.config.width as f32,
                gpu.renderer.config.height as f32,
            );
            let ray = Ray::from_screen(
                self.input.mouse_pos,
                screen_size,
                gpu.renderer.camera.view_projection(),
            );
            if let Some(hit) = crate::util::picking::pick_face_culled(&ray, &self.scene) {
                let face = &self.scene.layers[hit.layer_index].objects[hit.object_index].faces[hit.face_index];
                let obj = &self.scene.layers[hit.layer_index].objects[hit.object_index];
                if let Some(ts_idx) = obj.tileset_index {
                    self.scene.active_tileset = Some(ts_idx);
                    if let Some(tileset) = self.scene.tilesets.get(ts_idx) {
                        let uv = face.uvs[0];
                        let col = (uv.x * tileset.image_width as f32 / tileset.tile_width as f32).floor() as u32;
                        let row = (uv.y * tileset.image_height as f32 / tileset.tile_height as f32).floor() as u32;
                        self.draw_state.selected_tile = (col, row);
                        self.draw_state.selected_tile_end = (col, row);
                    }
                }
            }
        }

        // Edit mode: gizmo interaction (hover, drag start, drag update, drag commit)
        let mut gizmo_active = false;
        if self.tool_mode == ToolMode::Edit && !self.edit_state.selection.is_empty() && !self.input.space_held() {
            let screen_size = glam::Vec2::new(
                gpu.renderer.config.width as f32,
                gpu.renderer.config.height as f32,
            );
            let centroid = self.edit_state.selection.centroid(&self.scene);
            let cam_pos = gpu.renderer.camera.position;
            let scale = gizmo::gizmo_scale(centroid, cam_pos);
            let view_proj = gpu.renderer.camera.view_projection();
            let cam_fwd = (gpu.renderer.camera.target - cam_pos).normalize();

            // Hover detection (when not dragging)
            if self.edit_state.gizmo_drag.is_none() {
                self.edit_state.gizmo_hovered = gizmo::hit_test(
                    self.input.mouse_pos,
                    centroid,
                    scale,
                    self.edit_state.gizmo_mode,
                    view_proj,
                    screen_size,
                );
            }

            // Start gizmo drag on left click when hovering an axis
            if self.input.left_just_clicked
                && self.edit_state.gizmo_hovered != GizmoAxis::None
                && self.edit_state.gizmo_drag.is_none()
            {
                let ray = Ray::from_screen(self.input.mouse_pos, screen_size, view_proj);
                let axis = self.edit_state.gizmo_hovered;
                let start_point = match axis {
                    GizmoAxis::X | GizmoAxis::Y | GizmoAxis::Z => {
                        gizmo::project_ray_onto_axis(&ray, centroid, axis.direction(), cam_fwd)
                    }
                    GizmoAxis::XY | GizmoAxis::XZ | GizmoAxis::YZ => {
                        gizmo::project_ray_onto_plane(&ray, centroid, gizmo::plane_normal_for_axis(axis))
                    }
                    GizmoAxis::None => None,
                };

                if let Some(sp) = start_point {
                    let mut drag = gizmo::GizmoDrag::new(axis, sp, centroid);
                    if self.edit_state.gizmo_mode == GizmoMode::Rotate {
                        drag.start_angle = gizmo::compute_angle_on_axis(sp, centroid, axis.direction());
                    }
                    if self.edit_state.gizmo_mode == GizmoMode::Scale {
                        drag.start_distance = (sp - centroid).length().max(0.001);
                    }
                    self.edit_state.gizmo_drag = Some(drag);
                }
            }

            // Update active gizmo drag (take/put-back pattern for borrow safety)
            if let Some(mut drag) = self.edit_state.gizmo_drag.take() {
                gizmo_active = true;
                if self.input.left_pressed {
                    let ray = Ray::from_screen(self.input.mouse_pos, screen_size, view_proj);
                    match self.edit_state.gizmo_mode {
                        GizmoMode::Translate => {
                            let current = match drag.axis {
                                GizmoAxis::X | GizmoAxis::Y | GizmoAxis::Z => {
                                    gizmo::project_ray_onto_axis(&ray, drag.origin, drag.axis.direction(), cam_fwd)
                                }
                                GizmoAxis::XY | GizmoAxis::XZ | GizmoAxis::YZ => {
                                    gizmo::project_ray_onto_plane(&ray, drag.origin, gizmo::plane_normal_for_axis(drag.axis))
                                }
                                _ => None,
                            };
                            if let Some(cur) = current {
                                let total_delta = cur - drag.start_point;
                                let incremental = total_delta - drag.applied_delta;
                                if incremental.length_squared() > 1e-8 {
                                    Self::apply_translate_live(&self.edit_state.selection, &mut self.scene, incremental, &gpu.renderer.device);
                                    drag.applied_delta = total_delta;
                                }
                            }
                        }
                        GizmoMode::Rotate => {
                            let rot_axis = drag.axis.direction();
                            if let Some(cur) = gizmo::project_ray_onto_plane(&ray, drag.origin, rot_axis) {
                                let angle = gizmo::compute_angle_on_axis(cur, drag.origin, rot_axis);
                                let total_angle = angle - drag.start_angle;
                                let incremental = total_angle - drag.applied_angle;
                                if incremental.abs() > 1e-5 {
                                    Self::apply_rotate_live(&self.edit_state.selection, &mut self.scene, rot_axis, incremental, drag.origin, &gpu.renderer.device);
                                    drag.applied_angle = total_angle;
                                }
                            }
                        }
                        GizmoMode::Scale => {
                            let current = match drag.axis {
                                GizmoAxis::X | GizmoAxis::Y | GizmoAxis::Z => {
                                    gizmo::project_ray_onto_axis(&ray, drag.origin, drag.axis.direction(), cam_fwd)
                                }
                                _ => {
                                    gizmo::project_ray_onto_plane(&ray, drag.origin, gizmo::plane_normal_for_axis(drag.axis))
                                }
                            };
                            if let Some(cur) = current {
                                let dist = (cur - drag.origin).length().max(0.001);
                                let ratio = dist / drag.start_distance;
                                let new_scale = match drag.axis {
                                    GizmoAxis::X => glam::Vec3::new(ratio, 1.0, 1.0),
                                    GizmoAxis::Y => glam::Vec3::new(1.0, ratio, 1.0),
                                    GizmoAxis::Z => glam::Vec3::new(1.0, 1.0, ratio),
                                    _ => glam::Vec3::splat(ratio),
                                };
                                let undo_scale = glam::Vec3::new(
                                    1.0 / drag.applied_scale.x,
                                    1.0 / drag.applied_scale.y,
                                    1.0 / drag.applied_scale.z,
                                );
                                Self::apply_scale_live(&self.edit_state.selection, &mut self.scene, undo_scale, drag.origin, &gpu.renderer.device);
                                Self::apply_scale_live(&self.edit_state.selection, &mut self.scene, new_scale, drag.origin, &gpu.renderer.device);
                                drag.applied_scale = new_scale;
                            }
                        }
                    }
                    self.edit_state.gizmo_drag = Some(drag);
                } else {
                    // Mouse released — undo live preview, push command
                    // First, capture instance old transforms (post live-preview, about to be undone)
                    let has_instances = !self.edit_state.selection.instances.is_empty();

                    match self.edit_state.gizmo_mode {
                        GizmoMode::Translate => {
                            if drag.applied_delta.length_squared() > 1e-6 {
                                Self::apply_translate_live(&self.edit_state.selection, &mut self.scene, -drag.applied_delta, &gpu.renderer.device);
                                // After undo, current state = pre-drag. Capture old_transforms.
                                if has_instances {
                                    let targets = self.edit_state.selection.instances.clone();
                                    let old_transforms: Vec<_> = targets.iter().filter_map(|&(li, oi, ii)| {
                                        self.scene.layers.get(li)
                                            .and_then(|l| l.objects.get(oi))
                                            .and_then(|o| o.instances.get(ii))
                                            .map(|inst| (inst.position, inst.rotation, inst.scale))
                                    }).collect();
                                    let new_transforms: Vec<_> = old_transforms.iter().map(|&(pos, rot, scl)| {
                                        (pos + drag.applied_delta, rot, scl)
                                    }).collect();
                                    let cmd = commands::TransformInstance { targets, old_transforms, new_transforms };
                                    self.history.push(Box::new(cmd), &mut self.scene, &gpu.renderer.device);
                                }
                                let cmd = commands::TranslateSelection {
                                    faces: self.edit_state.selection.faces.clone(),
                                    objects: self.edit_state.selection.objects.clone(),
                                    vertices: self.edit_state.selection.vertices.clone(),
                                    delta: drag.applied_delta,
                                };
                                self.history.push(Box::new(cmd), &mut self.scene, &gpu.renderer.device);
                            }
                        }
                        GizmoMode::Rotate => {
                            if drag.applied_angle.abs() > 1e-5 {
                                Self::apply_rotate_live(&self.edit_state.selection, &mut self.scene, drag.axis.direction(), -drag.applied_angle, drag.origin, &gpu.renderer.device);
                                if has_instances {
                                    let quat = glam::Quat::from_axis_angle(drag.axis.direction(), drag.applied_angle);
                                    let targets = self.edit_state.selection.instances.clone();
                                    let old_transforms: Vec<_> = targets.iter().filter_map(|&(li, oi, ii)| {
                                        self.scene.layers.get(li)
                                            .and_then(|l| l.objects.get(oi))
                                            .and_then(|o| o.instances.get(ii))
                                            .map(|inst| (inst.position, inst.rotation, inst.scale))
                                    }).collect();
                                    let new_transforms: Vec<_> = old_transforms.iter().map(|&(pos, rot, scl)| {
                                        (quat * (pos - drag.origin) + drag.origin, quat * rot, scl)
                                    }).collect();
                                    let cmd = commands::TransformInstance { targets, old_transforms, new_transforms };
                                    self.history.push(Box::new(cmd), &mut self.scene, &gpu.renderer.device);
                                }
                                let cmd = commands::RotateSelection {
                                    faces: self.edit_state.selection.faces.clone(),
                                    objects: self.edit_state.selection.objects.clone(),
                                    vertices: self.edit_state.selection.vertices.clone(),
                                    axis: drag.axis.direction(),
                                    angle: drag.applied_angle,
                                    center: drag.origin,
                                };
                                self.history.push(Box::new(cmd), &mut self.scene, &gpu.renderer.device);
                            }
                        }
                        GizmoMode::Scale => {
                            if (drag.applied_scale - glam::Vec3::ONE).length_squared() > 1e-6 {
                                let undo_scale = glam::Vec3::new(
                                    1.0 / drag.applied_scale.x,
                                    1.0 / drag.applied_scale.y,
                                    1.0 / drag.applied_scale.z,
                                );
                                Self::apply_scale_live(&self.edit_state.selection, &mut self.scene, undo_scale, drag.origin, &gpu.renderer.device);
                                if has_instances {
                                    let targets = self.edit_state.selection.instances.clone();
                                    let old_transforms: Vec<_> = targets.iter().filter_map(|&(li, oi, ii)| {
                                        self.scene.layers.get(li)
                                            .and_then(|l| l.objects.get(oi))
                                            .and_then(|o| o.instances.get(ii))
                                            .map(|inst| (inst.position, inst.rotation, inst.scale))
                                    }).collect();
                                    let new_transforms: Vec<_> = old_transforms.iter().map(|&(pos, rot, scl)| {
                                        (drag.origin + (pos - drag.origin) * drag.applied_scale, rot, scl * drag.applied_scale)
                                    }).collect();
                                    let cmd = commands::TransformInstance { targets, old_transforms, new_transforms };
                                    self.history.push(Box::new(cmd), &mut self.scene, &gpu.renderer.device);
                                }
                                let cmd = commands::ScaleSelection {
                                    faces: self.edit_state.selection.faces.clone(),
                                    objects: self.edit_state.selection.objects.clone(),
                                    vertices: self.edit_state.selection.vertices.clone(),
                                    scale_factor: drag.applied_scale,
                                    center: drag.origin,
                                };
                                self.history.push(Box::new(cmd), &mut self.scene, &gpu.renderer.device);
                            }
                        }
                    }
                    // Auto-flatten UVs after gizmo transform
                    if self.settings.edit.auto_flatten_uvs {
                        auto_flatten_selection_uvs(
                            &mut self.scene,
                            &self.edit_state.selection.faces,
                            &self.edit_state.selection.objects,
                            &self.edit_state.selection.vertices,
                            &gpu.renderer.device,
                        );
                    }
                }
            }
        }

        // Edit mode: direct vertex/face drag (when gizmo is not hovered)
        let mut vertex_drag_active = false;
        if self.tool_mode == ToolMode::Edit && !self.input.space_held() && !gizmo_active
            && !self.edit_state.selection.is_empty()
        {
            let screen_size = glam::Vec2::new(
                gpu.renderer.config.width as f32,
                gpu.renderer.config.height as f32,
            );
            let view_proj = gpu.renderer.camera.view_projection();
            let cam_fwd = (gpu.renderer.camera.target - gpu.renderer.camera.position).normalize();

            // Start vertex drag on left click near a selected vertex/edge/face (when gizmo not hovered)
            if self.input.left_just_clicked
                && self.edit_state.gizmo_hovered == GizmoAxis::None
                && self.edit_state.vertex_drag.is_none()
            {
                let ray = Ray::from_screen(self.input.mouse_pos, screen_size, view_proj);
                let threshold = 12.0; // pixel threshold

                // Collect targets based on selection level
                let drag_info = match self.edit_state.selection_level {
                    crate::tools::edit::SelectionLevel::Vertex => {
                        find_vertex_drag_targets(&self.edit_state.selection.vertices, &self.scene, self.input.mouse_pos, view_proj, screen_size, threshold)
                    }
                    crate::tools::edit::SelectionLevel::Edge => {
                        find_edge_drag_targets(&self.edit_state.selection.edges, &self.scene, self.input.mouse_pos, view_proj, screen_size, threshold)
                    }
                    crate::tools::edit::SelectionLevel::Face => {
                        find_face_drag_targets(&self.edit_state.selection.faces, &self.scene, &ray)
                    }
                    crate::tools::edit::SelectionLevel::Object => None,
                };

                if let Some((start_world, targets)) = drag_info {
                    let plane_normal = cam_fwd;
                    self.edit_state.vertex_drag = Some(crate::tools::edit::VertexDrag {
                        plane_normal,
                        start_world,
                        targets,
                        applied_delta: glam::Vec3::ZERO,
                    });
                }
            }

            // Update active vertex drag
            if let Some(mut drag) = self.edit_state.vertex_drag.take() {
                vertex_drag_active = true;
                if self.input.left_pressed {
                    let ray = Ray::from_screen(self.input.mouse_pos, screen_size, view_proj);
                    if let Some(t) = ray.intersect_plane(drag.start_world, drag.plane_normal) {
                        let mut current = ray.point_at(t);

                        // Snap to grid with Ctrl
                        let ctrl = self.input.key_held(KeyCode::ControlLeft) || self.input.key_held(KeyCode::ControlRight);
                        if ctrl {
                            let grid = self.scene.grid_cell_size;
                            current.x = (current.x / grid).round() * grid;
                            current.y = (current.y / grid).round() * grid;
                            current.z = (current.z / grid).round() * grid;
                        }

                        let total_delta = current - drag.start_world;
                        let incremental = total_delta - drag.applied_delta;
                        if incremental.length_squared() > 1e-8 {
                            for &(li, oi, fi, vi, _) in &drag.targets {
                                self.scene.layers[li].objects[oi].faces[fi].positions[vi] += incremental;
                            }
                            // Rebuild affected GPU meshes
                            let mut rebuild = std::collections::HashSet::new();
                            for &(li, oi, _, _, _) in &drag.targets {
                                rebuild.insert((li, oi));
                            }
                            for (li, oi) in rebuild {
                                self.scene.layers[li].objects[oi].rebuild_gpu_mesh(&gpu.renderer.device);
                            }
                            drag.applied_delta = total_delta;
                        }
                    }
                    self.edit_state.vertex_drag = Some(drag);
                } else {
                    // Mouse released — undo preview and push command
                    if drag.applied_delta.length_squared() > 1e-6 {
                        // Undo the live preview
                        for &(li, oi, fi, vi, _) in &drag.targets {
                            self.scene.layers[li].objects[oi].faces[fi].positions[vi] -= drag.applied_delta;
                        }
                        let mut rebuild = std::collections::HashSet::new();
                        for &(li, oi, _, _, _) in &drag.targets {
                            rebuild.insert((li, oi));
                        }
                        for (li, oi) in &rebuild {
                            self.scene.layers[*li].objects[*oi].rebuild_gpu_mesh(&gpu.renderer.device);
                        }

                        // Push as TranslateSelection with the specific vertices
                        let vertices: Vec<(usize, usize, usize, usize)> = drag.targets.iter()
                            .map(|&(li, oi, fi, vi, _)| (li, oi, fi, vi))
                            .collect();
                        let flatten_verts = vertices.clone();
                        let cmd = commands::TranslateSelection {
                            faces: Vec::new(),
                            objects: Vec::new(),
                            vertices,
                            delta: drag.applied_delta,
                        };
                        self.history.push(Box::new(cmd), &mut self.scene, &gpu.renderer.device);
                        // Auto-flatten UVs after vertex drag
                        if self.settings.edit.auto_flatten_uvs {
                            auto_flatten_selection_uvs(
                                &mut self.scene,
                                &[],
                                &[],
                                &flatten_verts,
                                &gpu.renderer.device,
                            );
                        }
                    }
                }
            }
        }

        // Edit mode: marquee selection on drag release, or point-click selection
        if self.tool_mode == ToolMode::Edit && !self.input.space_held() && !gizmo_active && !vertex_drag_active {
            let shift = self.input.key_held(KeyCode::ShiftLeft) || self.input.key_held(KeyCode::ShiftRight);

            if self.input.left_just_released && self.input.is_dragging {
                // Marquee select
                if let Some(drag_start) = self.input.drag_start {
                    let screen_size = glam::Vec2::new(
                        gpu.renderer.config.width as f32,
                        gpu.renderer.config.height as f32,
                    );
                    self.edit_state.marquee_select(
                        &self.scene,
                        drag_start,
                        self.input.mouse_pos,
                        gpu.renderer.camera.view_projection(),
                        screen_size,
                        shift,
                    );
                }
            } else if self.input.left_just_clicked {
                // Point-click selection
                let screen_size = glam::Vec2::new(
                    gpu.renderer.config.width as f32,
                    gpu.renderer.config.height as f32,
                );
                let ray = Ray::from_screen(
                    self.input.mouse_pos,
                    screen_size,
                    gpu.renderer.camera.view_projection(),
                );
                self.edit_state.handle_click(&ray, &self.scene, shift);
            }
        }

        // Edit mode: translate selection by one grid step (with fine/coarse modifiers)
        if self.tool_mode == ToolMode::Edit && !self.edit_state.selection.is_empty() {
            let shift = self.input.key_held(KeyCode::ShiftLeft) || self.input.key_held(KeyCode::ShiftRight);
            let ctrl = self.input.key_held(KeyCode::ControlLeft) || self.input.key_held(KeyCode::ControlRight);
            let step = if shift {
                self.scene.grid_cell_size * 0.5  // Fine mode
            } else if ctrl {
                self.scene.grid_cell_size * 2.0  // Coarse mode
            } else {
                self.scene.grid_cell_size
            };
            let mut delta = glam::Vec3::ZERO;
            if self.input.key_just_pressed(KeyCode::ArrowUp) { delta.z -= step; }
            if self.input.key_just_pressed(KeyCode::ArrowDown) { delta.z += step; }
            if self.input.key_just_pressed(KeyCode::ArrowLeft) { delta.x -= step; }
            if self.input.key_just_pressed(KeyCode::ArrowRight) { delta.x += step; }
            if self.input.key_just_pressed(KeyCode::PageUp) { delta.y += step; }
            if self.input.key_just_pressed(KeyCode::PageDown) { delta.y -= step; }

            if delta != glam::Vec3::ZERO {
                let cmd = commands::TranslateSelection {
                    faces: self.edit_state.selection.faces.clone(),
                    objects: self.edit_state.selection.objects.clone(),
                    vertices: self.edit_state.selection.vertices.clone(),
                    delta,
                };
                self.history.push(Box::new(cmd), &mut self.scene, &gpu.renderer.device);
                if self.settings.edit.auto_flatten_uvs {
                    auto_flatten_selection_uvs(
                        &mut self.scene,
                        &self.edit_state.selection.faces,
                        &self.edit_state.selection.objects,
                        &self.edit_state.selection.vertices,
                        &gpu.renderer.device,
                    );
                }
            }
        }

        // Edit mode: Rotate selection (R = CW, Shift+R = CCW)
        let shift = self.input.key_held(KeyCode::ShiftLeft) || self.input.key_held(KeyCode::ShiftRight);
        if self.tool_mode == ToolMode::Edit
            && !self.edit_state.selection.is_empty()
            && self.input.key_just_pressed(KeyCode::KeyR)
            && !self.input.space_held()
        {
            let angle = if shift {
                -std::f32::consts::FRAC_PI_2
            } else {
                std::f32::consts::FRAC_PI_2
            };
            let center = self.edit_state.selection.centroid(&self.scene);
            let cmd = commands::RotateSelection {
                faces: self.edit_state.selection.faces.clone(),
                objects: self.edit_state.selection.objects.clone(),
                vertices: self.edit_state.selection.vertices.clone(),
                axis: glam::Vec3::Y,
                angle,
                center,
            };
            self.history.push(Box::new(cmd), &mut self.scene, &gpu.renderer.device);
        }

        // Edit mode: Flip normals (F)
        if self.tool_mode == ToolMode::Edit
            && !self.edit_state.selection.is_empty()
            && self.input.key_just_pressed(KeyCode::KeyF)
            && !self.input.space_held()
        {
            let cmd = commands::FlipNormals {
                faces: self.edit_state.selection.faces.clone(),
                objects: self.edit_state.selection.objects.clone(),
            };
            self.history.push(Box::new(cmd), &mut self.scene, &gpu.renderer.device);
        }

        // Edit mode: Extrude faces (E)
        if self.tool_mode == ToolMode::Edit
            && !self.edit_state.selection.faces.is_empty()
            && self.input.key_just_pressed(KeyCode::KeyE)
            && !self.input.space_held()
        {
            let cmd = commands::ExtrudeFaces::new(
                self.edit_state.selection.faces.clone(),
                self.scene.grid_cell_size,
            );
            self.history.push(Box::new(cmd), &mut self.scene, &gpu.renderer.device);
        }

        // Edit mode: Scale selection (+/- keys when GizmoMode::Scale)
        if self.tool_mode == ToolMode::Edit
            && !self.edit_state.selection.is_empty()
            && self.edit_state.gizmo_mode == GizmoMode::Scale
            && !self.input.space_held()
        {
            let mut scale_factor = None;
            if self.input.key_just_pressed(KeyCode::Equal) {
                scale_factor = Some(glam::Vec3::splat(1.1));
            }
            if self.input.key_just_pressed(KeyCode::Minus) {
                scale_factor = Some(glam::Vec3::splat(1.0 / 1.1));
            }
            if let Some(factor) = scale_factor {
                let center = self.edit_state.selection.centroid(&self.scene);
                let cmd = commands::ScaleSelection {
                    faces: self.edit_state.selection.faces.clone(),
                    objects: self.edit_state.selection.objects.clone(),
                    vertices: self.edit_state.selection.vertices.clone(),
                    scale_factor: factor,
                    center,
                };
                self.history.push(Box::new(cmd), &mut self.scene, &gpu.renderer.device);
            }
        }

        // Edit mode: Retile (T) — apply current tile UVs to selected faces
        if self.tool_mode == ToolMode::Edit
            && !self.edit_state.selection.faces.is_empty()
            && self.input.key_just_pressed(KeyCode::KeyT)
            && !self.input.space_held()
        {
            let new_uvs = self.draw_state.tile_uvs(&self.scene);
            let cmd = commands::RetileFaces {
                faces: self.edit_state.selection.faces.clone(),
                new_uvs,
                old_uvs: Vec::new(),
            };
            self.history.push(Box::new(cmd), &mut self.scene, &gpu.renderer.device);
        }

        // Edit mode: Center camera on selection (C)
        if self.tool_mode == ToolMode::Edit
            && !self.edit_state.selection.is_empty()
            && self.input.key_just_pressed(KeyCode::KeyC)
            && !self.input.space_held()
            && !self.input.key_held(KeyCode::ControlLeft)
            && !self.input.key_held(KeyCode::ControlRight)
        {
            let centroid = self.edit_state.selection.centroid(&self.scene);
            gpu.renderer.camera.center_on(centroid);
        }

        // Edit mode: Delete selection
        if self.tool_mode == ToolMode::Edit
            && self.keybindings.is_triggered(crate::keybindings::Action::Delete, &self.input)
            && !self.edit_state.selection.is_empty()
        {
            let mut removed_faces = Vec::new();
            for &(li, oi, fi) in &self.edit_state.selection.faces {
                if let Some(face) = self.scene.layers.get(li)
                    .and_then(|l| l.objects.get(oi))
                    .and_then(|o| o.faces.get(fi))
                {
                    removed_faces.push((li, oi, fi, face.clone()));
                }
            }
            let mut removed_objects = Vec::new();
            for &(li, oi) in &self.edit_state.selection.objects {
                if let Some(obj) = self.scene.layers.get(li).and_then(|l| l.objects.get(oi)) {
                    removed_objects.push((li, oi, obj.name.clone(), obj.faces.clone()));
                }
            }

            let cmd = commands::DeleteSelection { removed_faces, removed_objects };
            self.history.push(Box::new(cmd), &mut self.scene, &gpu.renderer.device);
            self.edit_state.selection.clear();
        }

        // Edit mode: Subdivide faces (Alt+D)
        let alt = self.input.key_held(KeyCode::AltLeft) || self.input.key_held(KeyCode::AltRight);
        if self.tool_mode == ToolMode::Edit
            && !self.edit_state.selection.faces.is_empty()
            && alt && self.input.key_just_pressed(KeyCode::KeyD)
        {
            let cmd = commands::SubdivideFaces::new(
                self.edit_state.selection.faces.clone(),
            );
            self.history.push(Box::new(cmd), &mut self.scene, &gpu.renderer.device);
            self.edit_state.selection.clear();
        }

        // Edit mode: Select connected faces (Ctrl+L)
        if self.tool_mode == ToolMode::Edit
            && !self.edit_state.selection.faces.is_empty()
            && self.input.key_held(KeyCode::ControlLeft) && self.input.key_just_pressed(KeyCode::KeyL)
        {
            self.edit_state.select_connected(&self.scene);
        }

        // Edit mode: Create Object from selection (Enter)
        if self.tool_mode == ToolMode::Edit
            && !self.edit_state.selection.faces.is_empty()
            && self.input.key_just_pressed(KeyCode::Enter)
            && !self.input.space_held()
        {
            let obj_count: usize = self.scene.layers.iter().map(|l| l.objects.len()).sum();
            let name = format!("Object {}", obj_count + 1);
            let cmd = commands::CreateObjectFromSelection::new(
                self.edit_state.selection.faces.clone(),
                self.scene.active_layer,
                name,
            );
            self.history.push(Box::new(cmd), &mut self.scene, &gpu.renderer.device);
            self.edit_state.selection.clear();
        }

        // Crosshair snap to vertex (Alt+C)
        if alt && self.input.key_just_pressed(KeyCode::KeyC)
            && !self.edit_state.selection.vertices.is_empty()
        {
            let &(li, oi, fi, vi) = &self.edit_state.selection.vertices[0];
            if let Some(pos) = self.scene.layers.get(li)
                .and_then(|l| l.objects.get(oi))
                .and_then(|o| o.faces.get(fi))
                .map(|f| f.positions[vi])
            {
                self.scene.crosshair_pos = pos;
            }
        } else if alt && self.input.key_just_pressed(KeyCode::KeyC)
            && !self.edit_state.selection.faces.is_empty()
        {
            let centroid = self.edit_state.selection.centroid(&self.scene);
            self.scene.crosshair_pos = centroid;
        }

        // Hide selected tiles (H)
        if self.tool_mode == ToolMode::Edit
            && !self.edit_state.selection.is_empty()
            && self.input.key_just_pressed(KeyCode::KeyH)
            && !self.input.space_held()
            && !shift
        {
            let mut to_hide = Vec::new();
            for &(li, oi, fi) in &self.edit_state.selection.faces {
                to_hide.push((li, oi, fi));
            }
            for &(li, oi) in &self.edit_state.selection.objects {
                if let Some(obj) = self.scene.layers.get(li).and_then(|l| l.objects.get(oi)) {
                    for fi in 0..obj.faces.len() {
                        to_hide.push((li, oi, fi));
                    }
                }
            }
            if !to_hide.is_empty() {
                let cmd = commands::HideFaces { faces: to_hide };
                self.history.push(Box::new(cmd), &mut self.scene, &gpu.renderer.device);
                self.edit_state.selection.clear();
            }
        }

        // Show all hidden tiles (Shift+H)
        if self.input.key_just_pressed(KeyCode::KeyH) && shift && !self.input.space_held() {
            let mut previously_hidden = Vec::new();
            for (li, layer) in self.scene.layers.iter().enumerate() {
                for (oi, obj) in layer.objects.iter().enumerate() {
                    for (fi, face) in obj.faces.iter().enumerate() {
                        if face.hidden {
                            previously_hidden.push((li, oi, fi));
                        }
                    }
                }
            }
            if !previously_hidden.is_empty() {
                let cmd = commands::ShowAllFaces { previously_hidden };
                self.history.push(Box::new(cmd), &mut self.scene, &gpu.renderer.device);
            }
        }

        // Edit mode: Merge vertices
        if self.tool_mode == ToolMode::Edit
            && self.keybindings.is_triggered(crate::keybindings::Action::MergeVertices, &self.input)
            && !self.input.space_held()
        {
            self.pending_action = Some(UiAction::MergeVertices);
        }

        // Undo/Redo hotkeys
        let ctrl = self.input.key_held(KeyCode::ControlLeft) || self.input.key_held(KeyCode::ControlRight);
        if self.keybindings.is_triggered(crate::keybindings::Action::Undo, &self.input) {
            self.history.undo(&mut self.scene, &gpu.renderer.device);
        }
        if self.keybindings.is_triggered(crate::keybindings::Action::Redo, &self.input) {
            self.history.redo(&mut self.scene, &gpu.renderer.device);
        }

        // New scene (confirm if unsaved)
        if self.keybindings.is_triggered(crate::keybindings::Action::NewScene, &self.input) {
            if self.history.dirty {
                self.confirm_dialog = Some(ConfirmDialog::NewScene);
            } else {
                self.pending_action = Some(UiAction::NewScene);
            }
        }

        if self.keybindings.is_triggered(crate::keybindings::Action::SaveScene, &self.input) {
            self.pending_action = Some(UiAction::SaveScene);
        }

        // Toggle floating tileset panel
        if self.keybindings.is_triggered(crate::keybindings::Action::ToggleFloatingTileset, &self.input) {
            self.draw_state.tileset_panel_floating = !self.draw_state.tileset_panel_floating;
        }
        if self.keybindings.is_triggered(crate::keybindings::Action::OpenScene, &self.input) {
            self.pending_action = Some(UiAction::OpenScene);
        }

        // Screenshot
        if self.keybindings.is_triggered(crate::keybindings::Action::Screenshot, &self.input) {
            self.screenshot_pending = true;
        }

        if self.keybindings.is_triggered(crate::keybindings::Action::ToggleUvPanel, &self.input) {
            self.uv_state.open = !self.uv_state.open;
        }

        // Select All / Deselect All
        if self.keybindings.is_triggered(crate::keybindings::Action::SelectAll, &self.input) {
            self.edit_state.select_all(&self.scene);
        }
        if self.keybindings.is_triggered(crate::keybindings::Action::DeselectAll, &self.input) {
            self.edit_state.selection.clear();
        }

        // Invert selection
        if self.keybindings.is_triggered(crate::keybindings::Action::InvertSelection, &self.input) {
            self.edit_state.invert_selection(&self.scene);
        }

        // Copy — copy selected faces to clipboard
        if self.keybindings.is_triggered(crate::keybindings::Action::Copy, &self.input) && !self.edit_state.selection.is_empty() {
            let mut faces = Vec::new();
            let mut tileset_index = None;

            for &(li, oi, fi) in &self.edit_state.selection.faces {
                if let Some(face) = self.scene.layers.get(li)
                    .and_then(|l| l.objects.get(oi))
                    .and_then(|o| o.faces.get(fi))
                {
                    faces.push(face.clone());
                    if tileset_index.is_none() {
                        tileset_index = self.scene.layers.get(li)
                            .and_then(|l| l.objects.get(oi))
                            .and_then(|o| o.tileset_index);
                    }
                }
            }

            for &(li, oi) in &self.edit_state.selection.objects {
                if let Some(obj) = self.scene.layers.get(li).and_then(|l| l.objects.get(oi)) {
                    for face in &obj.faces {
                        faces.push(face.clone());
                    }
                    if tileset_index.is_none() {
                        tileset_index = obj.tileset_index;
                    }
                }
            }

            if !faces.is_empty() {
                let mut sum = glam::Vec3::ZERO;
                let mut count = 0u32;
                for face in &faces {
                    for p in &face.positions {
                        sum += *p;
                        count += 1;
                    }
                }
                let centroid = if count > 0 { sum / count as f32 } else { glam::Vec3::ZERO };
                self.clipboard = Some(ClipboardData { faces, centroid, tileset_index });
            }
        }

        // Paste — paste clipboard at crosshair position
        if self.keybindings.is_triggered(crate::keybindings::Action::Paste, &self.input)
            && let Some(ref clip) = self.clipboard
        {
            let offset = self.scene.crosshair_pos - clip.centroid;
            let pasted_faces: Vec<Face> = clip.faces.iter().map(|f| {
                let mut face = f.clone();
                for pos in &mut face.positions {
                    *pos += offset;
                }
                face
            }).collect();

            if !pasted_faces.is_empty() {
                let layer_idx = self.scene.active_layer;
                let ts_idx = clip.tileset_index;
                let (object_idx, create_object) = crate::tools::draw::find_target_object(&self.scene, layer_idx, ts_idx);
                let cmd = commands::PlaceTile {
                    layer: layer_idx,
                    object: object_idx,
                    faces: pasted_faces,
                    create_object,
                    tileset_index: ts_idx,
                };
                self.history.push(Box::new(cmd), &mut self.scene, &gpu.renderer.device);
            }
        }

        // Camera bookmarks: Ctrl+Shift+1-5 to save, Ctrl+1-5 to recall
        let shift = self.input.key_held(KeyCode::ShiftLeft) || self.input.key_held(KeyCode::ShiftRight);
        let bookmark_keys = [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4, KeyCode::Digit5];
        if ctrl && self.tool_mode == ToolMode::Edit {
            for (i, key) in bookmark_keys.iter().enumerate() {
                if self.input.key_just_pressed(*key) {
                    if shift {
                        self.camera_bookmarks[i] = Some(gpu.renderer.camera.to_bookmark());
                    } else if let Some(ref bm) = self.camera_bookmarks[i] {
                        gpu.renderer.camera.apply_bookmark(bm);
                    }
                }
            }
        }

        // Compute placement preview (every frame in Draw mode)
        self.preview_faces.clear();
        if self.tool_mode == ToolMode::Draw
            && !self.input.space_held()
            && self.draw_state.tool != DrawTool::VertexColor
        {
            if let Some((start, normal)) = self.rect_fill_start {
                // Rectangle fill preview: compute fill from start to current mouse position
                let screen_size = glam::Vec2::new(
                    gpu.renderer.config.width as f32,
                    gpu.renderer.config.height as f32,
                );
                let ray = Ray::from_screen(
                    self.input.mouse_pos,
                    screen_size,
                    gpu.renderer.camera.view_projection(),
                );
                if let Some(t) = ray.intersect_plane(self.scene.crosshair_pos, normal) {
                    let pos = ray.point_at(t);
                    let cell = self.scene.grid_cell_size;
                    let end = glam::Vec3::new(
                        (pos.x / cell).round() * cell,
                        (pos.y / cell).round() * cell,
                        (pos.z / cell).round() * cell,
                    );
                    self.preview_faces = self.draw_state.compute_rect_fill(&self.scene, start, end, normal);
                }
            } else {
                let screen_size = glam::Vec2::new(
                    gpu.renderer.config.width as f32,
                    gpu.renderer.config.height as f32,
                );
                let ray = Ray::from_screen(
                    self.input.mouse_pos,
                    screen_size,
                    gpu.renderer.camera.view_projection(),
                );
                if let Some(result) = self.draw_state.compute_placement(&self.scene, &ray) {
                    self.preview_faces = result.faces;
                }
            }
        }

        // Compute hover highlight (every frame in Edit mode)
        self.hover_face = None;
        if self.tool_mode == ToolMode::Edit
            && !self.input.space_held()
            && !self.input.left_pressed
        {
            let screen_size = glam::Vec2::new(
                gpu.renderer.config.width as f32,
                gpu.renderer.config.height as f32,
            );
            let ray = Ray::from_screen(
                self.input.mouse_pos,
                screen_size,
                gpu.renderer.camera.view_projection(),
            );
            if let Some(hit) = crate::util::picking::pick_face(&ray, &self.scene) {
                self.hover_face = Some((hit.layer_index, hit.object_index, hit.face_index));
            }
        }

        // Build gizmo lines for rendering
        self.gizmo_lines.clear();
        if self.tool_mode == ToolMode::Edit && !self.edit_state.selection.is_empty() {
            let centroid = self.edit_state.selection.centroid(&self.scene);
            let cam_pos = gpu.renderer.camera.position;
            let scale = gizmo::gizmo_scale(centroid, cam_pos);
            let active_axis = self.edit_state.gizmo_drag.as_ref()
                .map(|d| d.axis)
                .unwrap_or(GizmoAxis::None);
            self.gizmo_lines = gizmo::build_gizmo_lines(
                centroid,
                scale,
                self.edit_state.gizmo_mode,
                self.edit_state.gizmo_hovered,
                active_axis,
            );
        }

        self.input.begin_frame();
    }

    fn redraw(&mut self) {
        self.process_input();
        let Some(gpu) = &mut self.gpu else { return };

        let output = match gpu.renderer.surface.get_current_texture() {
            Ok(output) => output,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                let size = gpu.window.inner_size();
                gpu.renderer.resize(size.width, size.height);
                return;
            }
            Err(e) => {
                log::error!("surface error: {e}");
                return;
            }
        };

        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Update window title with unsaved changes indicator
        self.has_unsaved_changes = self.history.dirty;
        let title = if self.has_unsaved_changes {
            "Cracktile 3D *"
        } else {
            "Cracktile 3D"
        };
        gpu.window.set_title(title);

        // Extract pending tileset for the egui closure
        let mut pending_tileset = self.pending_tileset.take();

        // Run egui
        let raw_input = gpu.egui_state.take_egui_input(&gpu.window);
        let egui_ctx = gpu.egui_state.egui_ctx().clone();
        let mut ui_result = UiResult { action: UiAction::None, property_commit: None };
        let mut light_settings = crate::ui::LightSettings {
            enabled: self.lighting_enabled,
            direction: gpu.renderer.light_direction,
            color: gpu.renderer.light_color,
            intensity: gpu.renderer.light_intensity,
            ambient: gpu.renderer.ambient_color,
        };
        let mut skybox_settings = crate::ui::SkyboxSettings {
            enabled: gpu.renderer.skybox.enabled,
            top_color: gpu.renderer.skybox.top_color,
            bottom_color: gpu.renderer.skybox.bottom_color,
            has_texture: gpu.renderer.skybox.has_texture(),
            use_texture: gpu.renderer.skybox.mode == crate::render::skybox::SkyboxMode::Equirect,
        };
        let screenshot_msg = if self.screenshot_flash > 0.0 {
            self.screenshot_last_path.as_deref()
        } else {
            None
        };
        let grid_cell_size = self.scene.grid_cell_size;
        let crosshair_y = self.scene.crosshair_pos.y;
        let full_output = egui_ctx.run(raw_input, |ctx| {
            ui_result = crate::ui::draw_ui(
                ctx,
                &mut self.scene,
                &mut self.tool_mode,
                &mut self.draw_state,
                &mut self.edit_state,
                &self.history,
                self.wireframe,
                &mut self.bg_color,
                self.has_unsaved_changes,
                &mut self.property_snapshot,
                &self.recent_files,
                &mut light_settings,
                &mut skybox_settings,
                &mut self.uv_state,
                &mut self.paint_state,
                screenshot_msg,
                gpu.renderer.camera.yaw,
                gpu.renderer.camera.pitch,
                &mut self.keybindings,
                &mut self.keybindings_editor_open,
                &mut self.settings,
                &mut self.settings_open,
                &mut self.settings_tab,
                gpu.renderer.backface_culling,
                &mut self.rulers_visible,
                gpu.renderer.camera.view_projection(),
                glam::Vec2::new(gpu.renderer.config.width as f32, gpu.renderer.config.height as f32),
                grid_cell_size,
                crosshair_y,
            );

            // Marquee selection visual feedback
            if self.tool_mode == ToolMode::Edit && self.input.is_dragging
                && let Some(start) = self.input.drag_start
            {
                let painter = ctx.layer_painter(egui::LayerId::new(
                    egui::Order::Foreground,
                    egui::Id::new("marquee"),
                ));
                let rect = egui::Rect::from_two_pos(
                    egui::pos2(start.x, start.y),
                    egui::pos2(self.input.mouse_pos.x, self.input.mouse_pos.y),
                );
                painter.rect(
                    rect,
                    0.0,
                    egui::Color32::from_rgba_unmultiplied(100, 150, 255, 30),
                    egui::Stroke::new(1.0, egui::Color32::from_rgb(100, 150, 255)),
                    egui::epaint::StrokeKind::Outside,
                );
            }

            // Confirm dialog (New Scene / Quit when unsaved)
            if let Some(ref dialog) = self.confirm_dialog {
                let title = match dialog {
                    ConfirmDialog::NewScene => "New Scene",
                    ConfirmDialog::Quit => "Quit",
                };
                let msg = "You have unsaved changes. Continue?";
                let mut confirmed = false;
                let mut cancelled = false;
                egui::Window::new(title)
                    .collapsible(false)
                    .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                    .show(ctx, |ui| {
                        ui.label(msg);
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            if ui.button("Yes, discard changes").clicked() { confirmed = true; }
                            if ui.button("Cancel").clicked() { cancelled = true; }
                        });
                    });
                if confirmed {
                    match self.confirm_dialog.take().unwrap() {
                        ConfirmDialog::NewScene => {
                            ui_result.action = UiAction::NewScene;
                        }
                        ConfirmDialog::Quit => {
                            ui_result.action = UiAction::Quit;
                        }
                    }
                }
                if cancelled {
                    self.confirm_dialog = None;
                }
            }

            // Tile size dialog
            if let Some(ref mut pending) = pending_tileset {
                let mut confirmed = false;
                let mut cancelled = false;
                egui::Window::new("Tile Size")
                    .collapsible(false)
                    .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                    .show(ctx, |ui| {
                        ui.horizontal(|ui| {
                            ui.label("Tile Width:");
                            ui.add(egui::DragValue::new(&mut pending.tile_width).range(1..=512));
                        });
                        ui.horizontal(|ui| {
                            ui.label("Tile Height:");
                            ui.add(egui::DragValue::new(&mut pending.tile_height).range(1..=512));
                        });
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            if ui.button("Load").clicked() { confirmed = true; }
                            if ui.button("Cancel").clicked() { cancelled = true; }
                        });
                    });
                if confirmed {
                    ui_result.action = UiAction::ConfirmTilesetLoad;
                }
                if cancelled {
                    pending_tileset = None;
                }
            }
        });

        // Put pending tileset back
        self.pending_tileset = pending_tileset;

        // Sync light settings back to renderer (may have been changed by UI)
        gpu.renderer.light_direction = light_settings.direction;
        gpu.renderer.light_color = light_settings.color;
        gpu.renderer.light_intensity = light_settings.intensity;
        gpu.renderer.ambient_color = light_settings.ambient;

        // Sync skybox settings back to renderer
        gpu.renderer.skybox.top_color = skybox_settings.top_color;
        gpu.renderer.skybox.bottom_color = skybox_settings.bottom_color;
        if skybox_settings.use_texture && gpu.renderer.skybox.has_texture() {
            gpu.renderer.skybox.mode = crate::render::skybox::SkyboxMode::Equirect;
        } else {
            gpu.renderer.skybox.mode = crate::render::skybox::SkyboxMode::Gradient;
        }

        gpu.egui_state.handle_platform_output(&gpu.window, full_output.platform_output);

        // Merge pending keyboard-triggered action with UI action
        let ui_action = if let Some(pending) = self.pending_action.take() {
            pending
        } else {
            ui_result.action
        };

        // Handle property edit commits from the properties panel
        if let Some(commit) = ui_result.property_commit {
            let cmd = commands::EditFaceProperty {
                face: commit.face,
                old_positions: commit.old_positions,
                old_uvs: commit.old_uvs,
                old_colors: commit.old_colors,
                new_positions: commit.new_positions,
                new_uvs: commit.new_uvs,
                new_colors: commit.new_colors,
            };
            self.history.push(Box::new(cmd), &mut self.scene, &gpu.renderer.device);
        }

        // Handle UI actions
        match ui_action {
            UiAction::NewScene => {
                self.scene = Scene::new();
                self.edit_state.selection.clear();
                self.history.clear();
                self.last_save_path = None;
                self.has_unsaved_changes = false;
                self.property_snapshot = None;
            }
            UiAction::Undo => {
                self.history.undo(&mut self.scene, &gpu.renderer.device);
            }
            UiAction::Redo => {
                self.history.redo(&mut self.scene, &gpu.renderer.device);
            }
            UiAction::LoadTileset => {
                let file = rfd::FileDialog::new()
                    .add_filter("Images", &["png", "jpg", "jpeg", "bmp"])
                    .set_title("Load Tileset Image")
                    .pick_file();

                if let Some(path) = file {
                    self.pending_tileset = Some(PendingTilesetLoad {
                        path,
                        tile_width: 16,
                        tile_height: 16,
                    });
                }
            }
            UiAction::ConfirmTilesetLoad => {
                if let Some(pending) = self.pending_tileset.take() {
                    Self::do_load_tileset(
                        &mut self.scene,
                        &mut gpu.egui_renderer,
                        &gpu.renderer,
                        &pending.path,
                        pending.tile_width,
                        pending.tile_height,
                    );
                }
            }
            UiAction::RemoveTileset(idx) => {
                // Check if any objects reference this tileset
                let refs: usize = self.scene.layers.iter()
                    .flat_map(|l| &l.objects)
                    .filter(|o| o.tileset_index == Some(idx))
                    .count();
                if refs > 0 {
                    log::warn!("Tileset {} is referenced by {refs} object(s) — clearing their tileset reference", idx);
                }
                // Clear tileset references on affected objects
                for layer in &mut self.scene.layers {
                    for obj in &mut layer.objects {
                        if obj.tileset_index == Some(idx) {
                            obj.tileset_index = None;
                        } else if let Some(ti) = obj.tileset_index {
                            // Shift down references above the removed index
                            if ti > idx {
                                obj.tileset_index = Some(ti - 1);
                            }
                        }
                    }
                }
                self.scene.tilesets.remove(idx);
                // Fix active_tileset
                if self.scene.tilesets.is_empty() {
                    self.scene.active_tileset = None;
                } else if let Some(active) = self.scene.active_tileset {
                    if active >= self.scene.tilesets.len() {
                        self.scene.active_tileset = Some(self.scene.tilesets.len() - 1);
                    } else if active > idx {
                        self.scene.active_tileset = Some(active - 1);
                    } else if active == idx {
                        self.scene.active_tileset = Some(0);
                    }
                }
                log::info!("Removed tileset {idx}");
            }
            UiAction::DuplicateTileset(idx) => {
                if let Some(ts) = self.scene.tilesets.get(idx)
                    && let Some(ref data) = ts.image_data
                {
                    let mat = ts.material.clone();
                    let mut dup = crate::tile::Tileset {
                        name: format!("{} (copy)", ts.name),
                        image_width: ts.image_width,
                        image_height: ts.image_height,
                        tile_width: ts.tile_width,
                        tile_height: ts.tile_height,
                        gpu_texture: None,
                        bind_group: None,
                        egui_texture_id: None,
                        image_data: Some(data.clone()),
                        material: mat,
                    };
                    // Recreate GPU resources from the cloned image data
                    Self::create_gpu_tileset(
                        &mut dup,
                        &gpu.renderer.device,
                        &gpu.renderer.queue,
                        &gpu.renderer.tile_bind_group_layout,
                    );
                    dup.register_with_egui(&mut gpu.egui_renderer, &gpu.renderer.device, &gpu.renderer.queue);
                    self.scene.tilesets.push(dup);
                    self.scene.active_tileset = Some(self.scene.tilesets.len() - 1);
                    log::info!("Duplicated tileset {idx}");
                }
            }
            UiAction::ReplaceTileset(idx) => {
                let file = rfd::FileDialog::new()
                    .add_filter("Images", &["png", "jpg", "jpeg", "bmp"])
                    .set_title("Replace Tileset Image")
                    .pick_file();

                if let Some(path) = file {
                    match image::open(&path) {
                        Ok(img) => {
                            let img = img.to_rgba8();
                            let (w, h) = img.dimensions();
                            let raw = img.into_raw();

                            if let Some(ts) = self.scene.tilesets.get_mut(idx) {
                                ts.image_width = w;
                                ts.image_height = h;
                                ts.image_data = Some(raw);
                                ts.name = path.file_stem()
                                    .map(|s| s.to_string_lossy().to_string())
                                    .unwrap_or_default();
                                // Recreate GPU resources
                                Self::create_gpu_tileset(
                                    ts,
                                    &gpu.renderer.device,
                                    &gpu.renderer.queue,
                                    &gpu.renderer.tile_bind_group_layout,
                                );
                                // Re-register with egui
                                ts.egui_texture_id = None;
                                ts.register_with_egui(&mut gpu.egui_renderer, &gpu.renderer.device, &gpu.renderer.queue);
                                // Rebuild all objects using this tileset to pick up texture changes
                                for layer in &mut self.scene.layers {
                                    for obj in &mut layer.objects {
                                        if obj.tileset_index == Some(idx) {
                                            obj.rebuild_gpu_mesh(&gpu.renderer.device);
                                        }
                                    }
                                }
                                log::info!("Replaced tileset {idx} with {:?}", path);
                            }
                        }
                        Err(e) => log::error!("Failed to load replacement image: {e}"),
                    }
                }
            }
            UiAction::ExportTileset(idx) => {
                if let Some(ts) = self.scene.tilesets.get(idx)
                    && let Some(ref data) = ts.image_data
                {
                    let file = rfd::FileDialog::new()
                        .add_filter("PNG Image", &["png"])
                        .set_title("Export Tileset")
                        .set_file_name(format!("{}.png", ts.name))
                        .save_file();

                    if let Some(path) = file {
                        match image::save_buffer(
                            &path,
                            data,
                            ts.image_width,
                            ts.image_height,
                            image::ColorType::Rgba8,
                        ) {
                            Ok(()) => log::info!("Exported tileset to {:?}", path),
                            Err(e) => log::error!("Failed to export tileset: {e}"),
                        }
                    }
                }
            }
            UiAction::RemoveUnusedTilesets => {
                // Find which tileset indices are referenced
                let mut used = std::collections::HashSet::new();
                for layer in &self.scene.layers {
                    for obj in &layer.objects {
                        if let Some(ti) = obj.tileset_index {
                            used.insert(ti);
                        }
                    }
                }
                // Remove from highest index to lowest to preserve indices
                let total = self.scene.tilesets.len();
                let mut removed = 0usize;
                for i in (0..total).rev() {
                    if !used.contains(&i) {
                        self.scene.tilesets.remove(i);
                        // Shift down references above this index
                        for layer in &mut self.scene.layers {
                            for obj in &mut layer.objects {
                                if let Some(ti) = obj.tileset_index
                                    && ti > i
                                {
                                    obj.tileset_index = Some(ti - 1);
                                }
                            }
                        }
                        // Update used set for subsequent removals
                        let mut new_used = std::collections::HashSet::new();
                        for &u in &used {
                            if u > i {
                                new_used.insert(u - 1);
                            } else {
                                new_used.insert(u);
                            }
                        }
                        used = new_used;
                        removed += 1;
                    }
                }
                // Fix active_tileset
                if self.scene.tilesets.is_empty() {
                    self.scene.active_tileset = None;
                } else if let Some(active) = self.scene.active_tileset
                    && active >= self.scene.tilesets.len()
                {
                    self.scene.active_tileset = Some(self.scene.tilesets.len() - 1);
                }
                log::info!("Removed {removed} unused tileset(s)");
            }
            UiAction::SaveScene => {
                if let Some(path) = self.last_save_path.clone() {
                    match crate::io::save_scene(&self.scene, &path) {
                        Ok(()) => {
                            log::info!("Saved scene to {:?}", path);
                            self.history.mark_saved();
                            self.recent_files.retain(|p| p != &path);
                            self.recent_files.insert(0, path);
                            self.recent_files.truncate(10);
                            crate::io::save_recent_files(&self.recent_files);
                        }
                        Err(e) => log::error!("Failed to save: {e}"),
                    }
                } else {
                    Self::do_save_scene(&self.scene, &mut self.last_save_path, &mut self.history, &mut self.recent_files);
                }
            }
            UiAction::SaveSceneAs => {
                Self::do_save_scene(&self.scene, &mut self.last_save_path, &mut self.history, &mut self.recent_files);
            }
            UiAction::OpenScene => {
                Self::do_open_scene(
                    &mut self.scene,
                    &mut self.edit_state,
                    &mut self.history,
                    &gpu.renderer,
                    &mut self.last_save_path,
                    &mut self.recent_files,
                );
            }
            UiAction::OpenRecentFile(idx) => {
                if let Some(path) = self.recent_files.get(idx).cloned() {
                    match crate::io::load_scene(&path) {
                        Ok(mut loaded) => {
                            for layer in &mut loaded.layers {
                                for obj in &mut layer.objects {
                                    obj.rebuild_gpu_mesh(&gpu.renderer.device);
                                }
                            }
                            self.scene = loaded;
                            self.edit_state.selection.clear();
                            self.history.clear();
                            self.last_save_path = Some(path.clone());
                            self.recent_files.retain(|p| p != &path);
                            self.recent_files.insert(0, path);
                            self.recent_files.truncate(10);
                            crate::io::save_recent_files(&self.recent_files);
                            log::info!("Opened scene from recent file");
                        }
                        Err(e) => log::error!("Failed to open: {e}"),
                    }
                }
            }
            UiAction::ExportObj => {
                Self::do_export_obj(&self.scene);
            }
            UiAction::ExportGlb => {
                Self::do_export_glb(&self.scene);
            }
            UiAction::ImportObj => {
                Self::do_import_obj(&mut self.scene, &mut self.history, &gpu.renderer);
            }
            UiAction::ImportGlb => {
                Self::do_import_glb(&mut self.scene, &mut self.history, &gpu.renderer);
            }
            UiAction::ExportGltf => {
                Self::do_export_gltf(&self.scene);
            }
            UiAction::ExportDae => {
                Self::do_export_dae(&self.scene);
            }
            UiAction::ImportGltf => {
                Self::do_import_gltf(&mut self.scene, &mut self.history, &gpu.renderer);
            }
            UiAction::ImportDae => {
                Self::do_import_dae(&mut self.scene, &mut self.history, &gpu.renderer);
            }
            UiAction::ToggleWireframe => {
                self.wireframe = !self.wireframe;
            }
            UiAction::ToggleLighting => {
                self.lighting_enabled = !self.lighting_enabled;
                gpu.renderer.set_lighting_enabled(self.lighting_enabled);
            }
            UiAction::ToggleSkybox => {
                gpu.renderer.skybox.enabled = !gpu.renderer.skybox.enabled;
            }
            UiAction::LoadSkyboxImage => {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Image", &["png", "jpg", "jpeg", "bmp", "hdr"])
                    .pick_file()
                    && let Err(e) = gpu.renderer.skybox.load_equirect(
                        &gpu.renderer.device,
                        &gpu.renderer.queue,
                        &path,
                    )
                {
                    eprintln!("Skybox load error: {e}");
                }
            }
            UiAction::SetSkyboxGradient => {
                // Handled by sync-back above
            }
            UiAction::TakeScreenshot => {
                self.screenshot_pending = true;
            }
            UiAction::ViewCubeClick(click) => {
                use crate::ui::viewcube::ViewCubeClick;
                match click {
                    ViewCubeClick::Front => gpu.renderer.camera.set_view_front(),
                    ViewCubeClick::Back => gpu.renderer.camera.set_view_back(),
                    ViewCubeClick::Left => gpu.renderer.camera.set_view_left(),
                    ViewCubeClick::Right => gpu.renderer.camera.set_view_right(),
                    ViewCubeClick::Top => gpu.renderer.camera.set_view_top(),
                    ViewCubeClick::Bottom => gpu.renderer.camera.set_view_bottom(),
                }
            }
            UiAction::ConfirmNewScene => {
                self.confirm_dialog = Some(ConfirmDialog::NewScene);
            }
            UiAction::RotateCW => {
                if !self.edit_state.selection.is_empty() {
                    let center = self.edit_state.selection.centroid(&self.scene);
                    let cmd = commands::RotateSelection {
                        faces: self.edit_state.selection.faces.clone(),
                        objects: self.edit_state.selection.objects.clone(),
                        vertices: self.edit_state.selection.vertices.clone(),
                        axis: glam::Vec3::Y,
                        angle: std::f32::consts::FRAC_PI_2,
                        center,
                    };
                    self.history.push(Box::new(cmd), &mut self.scene, &gpu.renderer.device);
                }
            }
            UiAction::RotateCCW => {
                if !self.edit_state.selection.is_empty() {
                    let center = self.edit_state.selection.centroid(&self.scene);
                    let cmd = commands::RotateSelection {
                        faces: self.edit_state.selection.faces.clone(),
                        objects: self.edit_state.selection.objects.clone(),
                        vertices: self.edit_state.selection.vertices.clone(),
                        axis: glam::Vec3::Y,
                        angle: -std::f32::consts::FRAC_PI_2,
                        center,
                    };
                    self.history.push(Box::new(cmd), &mut self.scene, &gpu.renderer.device);
                }
            }
            UiAction::FlipNormals => {
                if !self.edit_state.selection.is_empty() {
                    let cmd = commands::FlipNormals {
                        faces: self.edit_state.selection.faces.clone(),
                        objects: self.edit_state.selection.objects.clone(),
                    };
                    self.history.push(Box::new(cmd), &mut self.scene, &gpu.renderer.device);
                }
            }
            UiAction::ExtrudeFaces => {
                if !self.edit_state.selection.faces.is_empty() {
                    let cmd = commands::ExtrudeFaces::new(
                        self.edit_state.selection.faces.clone(),
                        self.scene.grid_cell_size,
                    );
                    self.history.push(Box::new(cmd), &mut self.scene, &gpu.renderer.device);
                }
            }
            UiAction::Retile => {
                if !self.edit_state.selection.faces.is_empty() {
                    let new_uvs = self.draw_state.tile_uvs(&self.scene);
                    let cmd = commands::RetileFaces {
                        faces: self.edit_state.selection.faces.clone(),
                        new_uvs,
                        old_uvs: Vec::new(),
                    };
                    self.history.push(Box::new(cmd), &mut self.scene, &gpu.renderer.device);
                }
            }
            UiAction::SubdivideFaces => {
                if !self.edit_state.selection.faces.is_empty() {
                    let cmd = commands::SubdivideFaces::new(
                        self.edit_state.selection.faces.clone(),
                    );
                    self.history.push(Box::new(cmd), &mut self.scene, &gpu.renderer.device);
                    self.edit_state.selection.clear();
                }
            }
            UiAction::TriangleDivide(diagonal) => {
                if !self.edit_state.selection.faces.is_empty() {
                    let cmd = commands::TriangleDivide::new(
                        self.edit_state.selection.faces.clone(),
                        diagonal,
                    );
                    self.history.push(Box::new(cmd), &mut self.scene, &gpu.renderer.device);
                    self.edit_state.selection.clear();
                }
            }
            UiAction::TriangleMerge => {
                // Find adjacent triangle pairs among selected faces
                let pairs = find_triangle_merge_pairs(&self.scene, &self.edit_state.selection.faces);
                if !pairs.is_empty() {
                    let cmd = commands::TriangleMerge::new(pairs);
                    self.history.push(Box::new(cmd), &mut self.scene, &gpu.renderer.device);
                    self.edit_state.selection.clear();
                }
            }
            UiAction::SelectTriangles => {
                // Select all degenerate quads (triangles) in the scene
                self.edit_state.selection.clear();
                for (li, layer) in self.scene.layers.iter().enumerate() {
                    for (oi, obj) in layer.objects.iter().enumerate() {
                        for (fi, face) in obj.faces.iter().enumerate() {
                            if is_degenerate_quad(face) {
                                self.edit_state.selection.faces.push((li, oi, fi));
                            }
                        }
                    }
                }
            }
            UiAction::PushVertices => {
                let moves = compute_push_pull_moves(&self.scene, &self.edit_state.selection, self.scene.grid_cell_size);
                if !moves.is_empty() {
                    let cmd = commands::MergeVertices { moves };
                    self.history.push(Box::new(cmd), &mut self.scene, &gpu.renderer.device);
                }
            }
            UiAction::PullVertices => {
                let moves = compute_push_pull_moves(&self.scene, &self.edit_state.selection, -self.scene.grid_cell_size);
                if !moves.is_empty() {
                    let cmd = commands::MergeVertices { moves };
                    self.history.push(Box::new(cmd), &mut self.scene, &gpu.renderer.device);
                }
            }
            UiAction::CenterToX => {
                let moves = compute_center_moves(&self.scene, &self.edit_state.selection, 0, self.scene.crosshair_pos.x);
                if !moves.is_empty() {
                    let cmd = commands::MergeVertices { moves };
                    self.history.push(Box::new(cmd), &mut self.scene, &gpu.renderer.device);
                }
            }
            UiAction::CenterToY => {
                let moves = compute_center_moves(&self.scene, &self.edit_state.selection, 1, self.scene.crosshair_pos.y);
                if !moves.is_empty() {
                    let cmd = commands::MergeVertices { moves };
                    self.history.push(Box::new(cmd), &mut self.scene, &gpu.renderer.device);
                }
            }
            UiAction::CenterToZ => {
                let moves = compute_center_moves(&self.scene, &self.edit_state.selection, 2, self.scene.crosshair_pos.z);
                if !moves.is_empty() {
                    let cmd = commands::MergeVertices { moves };
                    self.history.push(Box::new(cmd), &mut self.scene, &gpu.renderer.device);
                }
            }
            UiAction::StraightenVertices => {
                let moves = compute_straighten_moves(&self.scene, &self.edit_state.selection);
                if !moves.is_empty() {
                    let cmd = commands::MergeVertices { moves };
                    self.history.push(Box::new(cmd), &mut self.scene, &gpu.renderer.device);
                }
            }
            UiAction::DeleteSelection => {
                if !self.edit_state.selection.is_empty() {
                    let mut removed_faces = Vec::new();
                    for &(li, oi, fi) in &self.edit_state.selection.faces {
                        if let Some(face) = self.scene.layers.get(li)
                            .and_then(|l| l.objects.get(oi))
                            .and_then(|o| o.faces.get(fi))
                        {
                            removed_faces.push((li, oi, fi, face.clone()));
                        }
                    }
                    let mut removed_objects = Vec::new();
                    for &(li, oi) in &self.edit_state.selection.objects {
                        if let Some(obj) = self.scene.layers.get(li).and_then(|l| l.objects.get(oi)) {
                            removed_objects.push((li, oi, obj.name.clone(), obj.faces.clone()));
                        }
                    }
                    let cmd = commands::DeleteSelection { removed_faces, removed_objects };
                    self.history.push(Box::new(cmd), &mut self.scene, &gpu.renderer.device);
                    self.edit_state.selection.clear();
                }
            }
            UiAction::SelectAll => {
                self.edit_state.select_all(&self.scene);
            }
            UiAction::DeselectAll => {
                self.edit_state.selection.clear();
            }
            UiAction::InvertSelection => {
                self.edit_state.invert_selection(&self.scene);
            }
            UiAction::SelectByNormal => {
                let cam_fwd = (gpu.renderer.camera.target - gpu.renderer.camera.position).normalize();
                self.edit_state.select_by_normal(&self.scene, cam_fwd, 45.0);
            }
            UiAction::SelectOverlapping => {
                self.edit_state.select_overlapping(&self.scene);
            }
            UiAction::SelectByTilebrush => {
                // Get the current tilebrush UVs from draw_state
                if let Some(ts_idx) = self.scene.active_tileset
                    && let Some(ts) = self.scene.tilesets.get(ts_idx)
                {
                    let c0 = self.draw_state.selected_tile.0.min(self.draw_state.selected_tile_end.0);
                    let c1 = self.draw_state.selected_tile.0.max(self.draw_state.selected_tile_end.0);
                    let r0 = self.draw_state.selected_tile.1.min(self.draw_state.selected_tile_end.1);
                    let r1 = self.draw_state.selected_tile.1.max(self.draw_state.selected_tile_end.1);
                    let uvs = ts.tile_region_uvs(c0, r0, c1, r1);
                    self.edit_state.select_by_uvs(&self.scene, &uvs);
                }
            }
            UiAction::SelectEdgeLoop => {
                self.edit_state.select_edge_loop(&self.scene);
            }
            UiAction::SelectFacesFromVertices => {
                self.edit_state.select_faces_from_vertices(&self.scene);
            }
            // UV operations
            UiAction::UVRotateCW => {
                Self::apply_uv_op(&self.edit_state, &mut self.scene, &mut self.history, &gpu.renderer.device, |uvs| {
                    [uvs[3], uvs[0], uvs[1], uvs[2]]
                });
            }
            UiAction::UVRotateCCW => {
                Self::apply_uv_op(&self.edit_state, &mut self.scene, &mut self.history, &gpu.renderer.device, |uvs| {
                    [uvs[1], uvs[2], uvs[3], uvs[0]]
                });
            }
            UiAction::UVFlipH => {
                Self::apply_uv_op(&self.edit_state, &mut self.scene, &mut self.history, &gpu.renderer.device, |uvs| {
                    [uvs[1], uvs[0], uvs[3], uvs[2]]
                });
            }
            UiAction::UVFlipV => {
                Self::apply_uv_op(&self.edit_state, &mut self.scene, &mut self.history, &gpu.renderer.device, |uvs| {
                    [uvs[3], uvs[2], uvs[1], uvs[0]]
                });
            }
            // Geometry operations
            UiAction::MergeVertices => {
                Self::apply_merge_vertices(&self.edit_state, &mut self.scene, &mut self.history, &gpu.renderer.device);
            }
            UiAction::MirrorX => {
                Self::apply_mirror(&self.edit_state, &mut self.scene, &mut self.history, &gpu.renderer.device, 0);
            }
            UiAction::MirrorY => {
                Self::apply_mirror(&self.edit_state, &mut self.scene, &mut self.history, &gpu.renderer.device, 1);
            }
            UiAction::MirrorZ => {
                Self::apply_mirror(&self.edit_state, &mut self.scene, &mut self.history, &gpu.renderer.device, 2);
            }
            // Edge operations
            UiAction::SplitEdge => {
                if !self.edit_state.selection.edges.is_empty() {
                    let cmd = commands::SplitEdge::new(self.edit_state.selection.edges.clone());
                    self.history.push(Box::new(cmd), &mut self.scene, &gpu.renderer.device);
                    self.edit_state.selection.clear();
                }
            }
            UiAction::CollapseEdge => {
                if !self.edit_state.selection.edges.is_empty() {
                    let cmd = commands::CollapseEdge::new(self.edit_state.selection.edges.clone());
                    self.history.push(Box::new(cmd), &mut self.scene, &gpu.renderer.device);
                    self.edit_state.selection.clear();
                }
            }
            // Camera bookmarks
            UiAction::SaveBookmark(idx) => {
                if idx < 5 {
                    self.camera_bookmarks[idx] = Some(gpu.renderer.camera.to_bookmark());
                }
            }
            UiAction::RecallBookmark(idx) => {
                if let Some(Some(bm)) = self.camera_bookmarks.get(idx) {
                    gpu.renderer.camera.apply_bookmark(bm);
                }
            }
            UiAction::Quit => {
                std::process::exit(0);
            }
            UiAction::UvVertexDrag { targets, original_uvs, delta } => {
                // Build ManipulateUVs command from UV vertex drag
                let mut face_indices = Vec::new();
                let mut old_uvs_list = Vec::new();
                let mut new_uvs_list = Vec::new();
                // Group targets by face
                let mut face_map: std::collections::HashMap<(usize, usize, usize), [glam::Vec2; 4]> =
                    std::collections::HashMap::new();
                let mut face_old: std::collections::HashMap<(usize, usize, usize), [glam::Vec2; 4]> =
                    std::collections::HashMap::new();
                for (i, &(sel_idx, vi)) in targets.iter().enumerate() {
                    if let Some(&(li, oi, fi)) = self.edit_state.selection.faces.get(sel_idx)
                        && let Some(face) = self.scene.layers.get(li)
                            .and_then(|l| l.objects.get(oi))
                            .and_then(|o| o.faces.get(fi))
                    {
                        let key = (li, oi, fi);
                        face_old.entry(key).or_insert(face.uvs);
                        let entry = face_map.entry(key).or_insert(face.uvs);
                        entry[vi] = original_uvs[i] + delta;
                    }
                }
                for (key, new_uvs) in &face_map {
                    if let Some(old_uvs) = face_old.get(key) {
                        face_indices.push(*key);
                        old_uvs_list.push(*old_uvs);
                        new_uvs_list.push(*new_uvs);
                    }
                }
                if !face_indices.is_empty() {
                    let cmd = commands::ManipulateUVs {
                        faces: face_indices,
                        old_uvs: old_uvs_list,
                        new_uvs: new_uvs_list,
                    };
                    self.history.push(Box::new(cmd), &mut self.scene, &gpu.renderer.device);
                }
            }
            UiAction::RebuildMaterial(idx) => {
                if let Some(tileset) = self.scene.tilesets.get_mut(idx) {
                    tileset.rebuild_bind_group(
                        &gpu.renderer.device,
                        &gpu.renderer.tile_bind_group_layout,
                    );
                    self.has_unsaved_changes = true;
                }
            }
            UiAction::CreatePrefab => {
                // Gather selected faces into a new prefab
                let mut faces = Vec::new();
                let mut ts_idx = None;
                for &(li, oi, fi) in &self.edit_state.selection.faces {
                    if let Some(face) = self.scene.layers.get(li)
                        .and_then(|l| l.objects.get(oi))
                        .and_then(|o| o.faces.get(fi))
                    {
                        faces.push(face.clone());
                        if ts_idx.is_none() {
                            ts_idx = self.scene.layers.get(li)
                                .and_then(|l| l.objects.get(oi))
                                .and_then(|o| o.tileset_index);
                        }
                    }
                }
                if !faces.is_empty() {
                    let n = self.scene.prefabs.len() + 1;
                    let prefab = crate::scene::Prefab::from_faces(
                        format!("Prefab {n}"),
                        faces,
                        ts_idx,
                    );
                    self.scene.prefabs.push(prefab);
                    self.scene.active_prefab = Some(self.scene.prefabs.len() - 1);
                    self.has_unsaved_changes = true;
                    log::info!("Created prefab from {} selected faces", self.edit_state.selection.faces.len());
                }
            }
            UiAction::DeletePrefab(idx) => {
                if idx < self.scene.prefabs.len() {
                    self.scene.prefabs.remove(idx);
                    if let Some(ref mut active) = self.scene.active_prefab
                        && *active >= self.scene.prefabs.len()
                    {
                        self.scene.active_prefab = if self.scene.prefabs.is_empty() {
                            None
                        } else {
                            Some(self.scene.prefabs.len() - 1)
                        };
                    }
                    self.has_unsaved_changes = true;
                }
            }
            UiAction::AddBone => {
                // Add a new bone at the crosshair position, extending upward
                let head = self.scene.crosshair_pos;
                // If a bone is selected, make the new bone a child extending from the selected bone's tail
                let selected = self.scene.skeleton.selected_indices();
                let (actual_head, parent) = if selected.len() == 1 {
                    let parent_idx = selected[0];
                    let parent_tail = self.scene.skeleton.bones[parent_idx].posed_tail();
                    (parent_tail, Some(parent_idx))
                } else {
                    (head, None)
                };
                let actual_tail = actual_head + glam::Vec3::Y * self.scene.grid_cell_size;
                let n = self.scene.skeleton.bones.len() + 1;
                let bone = crate::bones::Bone::new(
                    format!("Bone {n}"),
                    actual_head,
                    actual_tail,
                    parent,
                );
                let idx = self.scene.skeleton.add_bone(bone);
                self.scene.skeleton.select_bone(idx, false);
                self.has_unsaved_changes = true;
            }
            UiAction::DeleteBone(idx) => {
                if idx < self.scene.skeleton.bones.len() {
                    // Update parent references for children of deleted bone
                    let parent_of_deleted = self.scene.skeleton.bones[idx].parent;
                    for bone in &mut self.scene.skeleton.bones {
                        if bone.parent == Some(idx) {
                            bone.parent = parent_of_deleted;
                        }
                        // Shift indices for bones after deleted one
                        if let Some(ref mut p) = bone.parent
                            && *p > idx
                        {
                            *p -= 1;
                        }
                    }
                    self.scene.skeleton.bones.remove(idx);
                    self.has_unsaved_changes = true;
                }
            }
            UiAction::DeconstructPrefab => {
                // Placeholder — currently prefabs are placed as normal faces
                // so deconstruction is the default behavior
            }
            UiAction::CreateInstance => {
                // Create an instance from selected objects
                if !self.edit_state.selection.objects.is_empty() {
                    for &(li, oi) in &self.edit_state.selection.objects.clone() {
                        let inst = crate::scene::Instance {
                            name: format!("{} (inst)", self.scene.layers[li].objects[oi].name),
                            position: glam::Vec3::new(1.0, 0.0, 0.0),
                            ..Default::default()
                        };
                        self.history.push(Box::new(crate::history::commands::CreateInstance {
                            layer: li, object: oi, instance: inst,
                        }), &mut self.scene, &gpu.renderer.device);
                    }
                    self.has_unsaved_changes = true;
                }
            }
            UiAction::DeleteInstance => {
                // Delete selected instances
                let mut targets: Vec<(usize, usize, usize)> = self.edit_state.selection.instances.clone();
                // Sort in reverse to avoid index shifting
                targets.sort_by(|a, b| b.2.cmp(&a.2).then_with(|| b.1.cmp(&a.1)).then_with(|| b.0.cmp(&a.0)));
                for (li, oi, ii) in targets {
                    self.history.push(Box::new(crate::history::commands::DeleteInstance {
                        layer: li, object: oi, instance_index: ii, stored: None,
                    }), &mut self.scene, &gpu.renderer.device);
                }
                self.edit_state.selection.instances.clear();
                self.has_unsaved_changes = true;
            }
            UiAction::DeconstructInstance => {
                // Deconstruct selected instances into independent objects
                let mut targets: Vec<(usize, usize, usize)> = self.edit_state.selection.instances.clone();
                targets.sort_by(|a, b| b.2.cmp(&a.2).then_with(|| b.1.cmp(&a.1)).then_with(|| b.0.cmp(&a.0)));
                for (li, oi, ii) in targets {
                    self.history.push(Box::new(
                        crate::history::commands::DeconstructInstance::new(li, oi, ii)
                    ), &mut self.scene, &gpu.renderer.device);
                }
                self.edit_state.selection.instances.clear();
                self.has_unsaved_changes = true;
            }
            UiAction::RenamePrefab(idx, ref new_name) => {
                if let Some(prefab) = self.scene.prefabs.get_mut(idx) {
                    prefab.name = new_name.clone();
                    self.has_unsaved_changes = true;
                }
            }
            UiAction::OpenPaintEditor => {
                if let Some(idx) = self.scene.active_tileset
                    && let Some(tileset) = self.scene.tilesets.get(idx)
                    && let Some(ref image_data) = tileset.image_data
                {
                    self.paint_state.load_tileset(idx, image_data.clone(), tileset.image_width, tileset.image_height);
                    self.paint_state.open = true;
                }
            }
            UiAction::PaintSyncToGpu => {
                if let Some(idx) = self.paint_state.tileset_index
                    && let Some(tileset) = self.scene.tilesets.get_mut(idx)
                {
                    // Update tileset image_data from paint buffer
                    tileset.image_data = Some(self.paint_state.pixels.clone());

                    // Re-upload to wgpu texture
                    if let Some(ref texture) = tileset.gpu_texture {
                        gpu.renderer.queue.write_texture(
                            wgpu::TexelCopyTextureInfo {
                                texture,
                                mip_level: 0,
                                origin: wgpu::Origin3d::ZERO,
                                aspect: wgpu::TextureAspect::All,
                            },
                            &self.paint_state.pixels,
                            wgpu::TexelCopyBufferLayout {
                                offset: 0,
                                bytes_per_row: Some(4 * tileset.image_width),
                                rows_per_image: Some(tileset.image_height),
                            },
                            wgpu::Extent3d {
                                width: tileset.image_width,
                                height: tileset.image_height,
                                depth_or_array_layers: 1,
                            },
                        );
                    }

                    // Re-register egui texture with updated data
                    // Unregister old, then re-register
                    if let Some(old_id) = tileset.egui_texture_id.take() {
                        gpu.egui_renderer.free_texture(&old_id);
                    }
                    tileset.register_with_egui(
                        &mut gpu.egui_renderer,
                        &gpu.renderer.device,
                        &gpu.renderer.queue,
                    );

                    self.paint_state.dirty = false;
                    self.has_unsaved_changes = true;
                }
            }
            UiAction::OpenKeybindingsEditor => {
                self.keybindings_editor_open = true;
            }
            UiAction::ResetKeybindings => {
                self.keybindings = Keybindings::defaults();
                self.keybindings.save();
            }
            UiAction::OpenSettings => {
                self.settings_open = true;
            }
            UiAction::ResetSettings => {
                self.settings = crate::settings::Settings::default();
                self.settings.save();
                self.bg_color = self.settings.display.bg_color;
            }
            UiAction::ToggleBackfaceCulling => {
                gpu.renderer.backface_culling = !gpu.renderer.backface_culling;
            }
            UiAction::None => {}
        }

        // Sync bg_color to/from settings (View menu edits bg_color directly)
        self.settings.display.bg_color = self.bg_color;

        // Rebuild GPU meshes for objects dirtied by property edits
        if !self.scene.dirty_objects.is_empty() {
            let dirty: std::collections::HashSet<(usize, usize)> = self.scene.dirty_objects.drain(..).collect();
            for (li, oi) in dirty {
                if let Some(obj) = self.scene.layers.get_mut(li).and_then(|l| l.objects.get_mut(oi)) {
                    obj.rebuild_gpu_mesh(&gpu.renderer.device);
                }
            }
        }

        let paint_jobs = egui_ctx.tessellate(full_output.shapes, full_output.pixels_per_point);
        let screen_descriptor = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [
                gpu.renderer.config.width,
                gpu.renderer.config.height,
            ],
            pixels_per_point: full_output.pixels_per_point,
        };

        // Update egui textures
        for (id, delta) in &full_output.textures_delta.set {
            gpu.egui_renderer.update_texture(&gpu.renderer.device, &gpu.renderer.queue, *id, delta);
        }

        // Upload per-frame data before render pass
        gpu.renderer.prepare_frame(&self.scene);

        // Main 3D render pass
        {
            let mut encoder = gpu.renderer.device.create_command_encoder(
                &wgpu::CommandEncoderDescriptor { label: Some("scene_encoder") },
            );
            {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("main_pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                r: self.bg_color[0] as f64,
                                g: self.bg_color[1] as f64,
                                b: self.bg_color[2] as f64,
                                a: 1.0,
                            }),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &gpu.renderer.depth_view,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(1.0),
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }),
                    ..Default::default()
                });

                gpu.renderer.render_scene(&mut pass, &self.scene, &self.input, self.wireframe);
                let preview_color = if self.draw_state.tool == DrawTool::Block && self.draw_state.block_subtract {
                    Some([1.0, 0.3, 0.3, 1.0]) // Red for subtract
                } else {
                    None // Default green
                };
                gpu.renderer.render_preview(&mut pass, &self.preview_faces, preview_color);
                gpu.renderer.render_hover(&mut pass, &self.scene, self.hover_face);
                gpu.renderer.render_selection(&mut pass, &self.scene, &self.edit_state.selection);
                gpu.renderer.render_gizmo(&mut pass, &self.gizmo_lines);
                gpu.renderer.render_bones(&mut pass, &self.scene.skeleton);
            }
            gpu.renderer.queue.submit(std::iter::once(encoder.finish()));
        }

        // Egui render pass (separate encoder so egui owns the pass)
        {
            let mut encoder = gpu.renderer.device.create_command_encoder(
                &wgpu::CommandEncoderDescriptor { label: Some("egui_encoder") },
            );

            gpu.egui_renderer.update_buffers(
                &gpu.renderer.device,
                &gpu.renderer.queue,
                &mut encoder,
                &paint_jobs,
                &screen_descriptor,
            );

            {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("egui_pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    ..Default::default()
                });
                // SAFETY: The render pass is dropped before encoder.finish() is called.
                let pass_static: &mut wgpu::RenderPass<'static> =
                    unsafe { std::mem::transmute(&mut pass) };
                gpu.egui_renderer.render(pass_static, &paint_jobs, &screen_descriptor);
            }

            gpu.renderer.queue.submit(std::iter::once(encoder.finish()));
        }

        // Capture screenshot if requested (before present)
        if self.screenshot_pending {
            self.screenshot_pending = false;
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let dir = dirs_or_home("Pictures").join("Cracktile3D");
            let filename = format!("screenshot_{timestamp}.png");
            let path = dir.join(&filename);
            match gpu.renderer.capture_screenshot(&output.texture, &path) {
                Ok(()) => {
                    log::info!("Screenshot saved to {}", path.display());
                    self.screenshot_flash = 3.0;
                    self.screenshot_last_path = Some(format!("Screenshot: {}", path.display()));
                }
                Err(e) => {
                    log::error!("Screenshot failed: {e}");
                    self.screenshot_flash = 3.0;
                    self.screenshot_last_path = Some(format!("Screenshot failed: {e}"));
                }
            }
        }

        // Tick screenshot flash timer
        if self.screenshot_flash > 0.0 {
            self.screenshot_flash -= 1.0 / 60.0;
        }

        output.present();

        // Free egui textures
        for id in &full_output.textures_delta.free {
            gpu.egui_renderer.free_texture(id);
        }
    }

    fn do_load_tileset(
        scene: &mut Scene,
        egui_renderer: &mut egui_wgpu::Renderer,
        renderer: &Renderer,
        path: &std::path::Path,
        tile_w: u32,
        tile_h: u32,
    ) {
        match crate::tile::Tileset::load(
            &renderer.device,
            &renderer.queue,
            &renderer.tile_bind_group_layout,
            path,
            tile_w,
            tile_h,
        ) {
            Ok(mut tileset) => {
                tileset.register_with_egui(egui_renderer, &renderer.device, &renderer.queue);
                scene.tilesets.push(tileset);
                scene.active_tileset = Some(scene.tilesets.len() - 1);
                log::info!("Loaded tileset from {:?} ({}x{} tiles)", path, tile_w, tile_h);
            }
            Err(e) => {
                log::error!("Failed to load tileset: {e}");
            }
        }
    }

    /// Create GPU texture and bind group for a tileset from its image_data.
    fn create_gpu_tileset(
        ts: &mut crate::tile::Tileset,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bind_group_layout: &wgpu::BindGroupLayout,
    ) {
        let Some(ref data) = ts.image_data else { return };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("tileset_texture"),
            size: wgpu::Extent3d {
                width: ts.image_width,
                height: ts.image_height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * ts.image_width),
                rows_per_image: Some(ts.image_height),
            },
            wgpu::Extent3d {
                width: ts.image_width,
                height: ts.image_height,
                depth_or_array_layers: 1,
            },
        );
        ts.gpu_texture = Some(texture);
        ts.rebuild_bind_group(device, bind_group_layout);
    }

    fn do_save_scene(scene: &Scene, last_save_path: &mut Option<std::path::PathBuf>, history: &mut History, recent_files: &mut Vec<std::path::PathBuf>) {
        let file = rfd::FileDialog::new()
            .add_filter("Cracktile 3D", &["ct3d"])
            .set_title("Save Scene")
            .save_file();

        if let Some(path) = file {
            match crate::io::save_scene(scene, &path) {
                Ok(()) => {
                    log::info!("Saved scene to {:?}", path);
                    *last_save_path = Some(path.clone());
                    history.mark_saved();
                    recent_files.retain(|p| p != &path);
                    recent_files.insert(0, path);
                    recent_files.truncate(10);
                    crate::io::save_recent_files(recent_files);
                }
                Err(e) => log::error!("Failed to save: {e}"),
            }
        }
    }

    fn do_open_scene(
        scene: &mut Scene,
        edit_state: &mut EditState,
        history: &mut History,
        renderer: &Renderer,
        last_save_path: &mut Option<std::path::PathBuf>,
        recent_files: &mut Vec<std::path::PathBuf>,
    ) {
        let file = rfd::FileDialog::new()
            .add_filter("Cracktile 3D", &["ct3d"])
            .set_title("Open Scene")
            .pick_file();

        if let Some(path) = file {
            match crate::io::load_scene(&path) {
                Ok(mut loaded) => {
                    for layer in &mut loaded.layers {
                        for obj in &mut layer.objects {
                            obj.rebuild_gpu_mesh(&renderer.device);
                        }
                    }
                    *scene = loaded;
                    edit_state.selection.clear();
                    history.clear();
                    *last_save_path = Some(path.clone());
                    recent_files.retain(|p| p != &path);
                    recent_files.insert(0, path);
                    recent_files.truncate(10);
                    crate::io::save_recent_files(recent_files);
                    log::info!("Opened scene");
                }
                Err(e) => log::error!("Failed to open: {e}"),
            }
        }
    }

    fn do_export_glb(scene: &Scene) {
        let file = rfd::FileDialog::new()
            .add_filter("glTF Binary", &["glb"])
            .set_title("Export GLB")
            .save_file();

        if let Some(path) = file {
            match crate::io::export_glb(scene, &path) {
                Ok(()) => log::info!("Exported GLB to {:?}", path),
                Err(e) => log::error!("Failed to export GLB: {e}"),
            }
        }
    }

    fn do_export_obj(scene: &Scene) {
        let file = rfd::FileDialog::new()
            .add_filter("Wavefront OBJ", &["obj"])
            .set_title("Export OBJ")
            .save_file();

        if let Some(path) = file {
            match crate::io::export_obj(scene, &path) {
                Ok(()) => log::info!("Exported OBJ to {:?}", path),
                Err(e) => log::error!("Failed to export OBJ: {e}"),
            }
        }
    }

    fn do_import_obj(scene: &mut Scene, history: &mut History, renderer: &Renderer) {
        let file = rfd::FileDialog::new()
            .add_filter("Wavefront OBJ", &["obj"])
            .set_title("Import OBJ")
            .pick_file();

        if let Some(path) = file {
            match crate::io::import_obj(&path) {
                Ok(objects) => {
                    Self::import_objects(scene, history, renderer, objects);
                    log::info!("Imported OBJ from {:?}", path);
                }
                Err(e) => log::error!("Failed to import OBJ: {e}"),
            }
        }
    }

    fn do_import_glb(scene: &mut Scene, history: &mut History, renderer: &Renderer) {
        let file = rfd::FileDialog::new()
            .add_filter("glTF Binary", &["glb"])
            .set_title("Import GLB")
            .pick_file();

        if let Some(path) = file {
            match crate::io::import_glb(&path) {
                Ok(objects) => {
                    Self::import_objects(scene, history, renderer, objects);
                    log::info!("Imported GLB from {:?}", path);
                }
                Err(e) => log::error!("Failed to import GLB: {e}"),
            }
        }
    }

    fn do_export_gltf(scene: &Scene) {
        let file = rfd::FileDialog::new()
            .add_filter("glTF JSON", &["gltf"])
            .set_title("Export glTF")
            .save_file();

        if let Some(path) = file {
            match crate::io::export_gltf(scene, &path) {
                Ok(()) => log::info!("Exported glTF to {:?}", path),
                Err(e) => log::error!("Failed to export glTF: {e}"),
            }
        }
    }

    fn do_export_dae(scene: &Scene) {
        let file = rfd::FileDialog::new()
            .add_filter("Collada", &["dae"])
            .set_title("Export DAE")
            .save_file();

        if let Some(path) = file {
            match crate::io::export_dae(scene, &path) {
                Ok(()) => log::info!("Exported DAE to {:?}", path),
                Err(e) => log::error!("Failed to export DAE: {e}"),
            }
        }
    }

    fn do_import_gltf(scene: &mut Scene, history: &mut History, renderer: &Renderer) {
        let file = rfd::FileDialog::new()
            .add_filter("glTF JSON", &["gltf"])
            .set_title("Import glTF")
            .pick_file();

        if let Some(path) = file {
            match crate::io::import_gltf(&path) {
                Ok(objects) => {
                    Self::import_objects(scene, history, renderer, objects);
                    log::info!("Imported glTF from {:?}", path);
                }
                Err(e) => log::error!("Failed to import glTF: {e}"),
            }
        }
    }

    fn do_import_dae(scene: &mut Scene, history: &mut History, renderer: &Renderer) {
        let file = rfd::FileDialog::new()
            .add_filter("Collada", &["dae"])
            .set_title("Import DAE")
            .pick_file();

        if let Some(path) = file {
            match crate::io::import_dae(&path) {
                Ok(objects) => {
                    Self::import_objects(scene, history, renderer, objects);
                    log::info!("Imported DAE from {:?}", path);
                }
                Err(e) => log::error!("Failed to import DAE: {e}"),
            }
        }
    }

    fn import_objects(
        scene: &mut Scene,
        history: &mut History,
        renderer: &Renderer,
        objects: Vec<(Vec<Face>, Option<String>)>,
    ) {
        for (faces, name) in objects {
            let layer_idx = scene.active_layer;
            let (obj_idx, create) = crate::tools::draw::find_target_object(scene, layer_idx, None);
            let cmd = commands::PlaceTile {
                layer: layer_idx,
                object: obj_idx,
                faces,
                create_object: create,
                tileset_index: None,
            };
            history.push(Box::new(cmd), scene, &renderer.device);
            if let Some(obj) = scene.layers.get_mut(layer_idx).and_then(|l| l.objects.get_mut(obj_idx))
                && let Some(n) = name
            {
                obj.name = n;
            }
        }
    }

    fn apply_uv_op(
        edit_state: &EditState,
        scene: &mut Scene,
        history: &mut History,
        device: &wgpu::Device,
        transform: impl Fn([glam::Vec2; 4]) -> [glam::Vec2; 4],
    ) {
        if edit_state.selection.faces.is_empty() { return; }
        let mut face_indices = Vec::new();
        let mut old_uvs = Vec::new();
        let mut new_uvs = Vec::new();
        for &(li, oi, fi) in &edit_state.selection.faces {
            if let Some(face) = scene.layers.get(li)
                .and_then(|l| l.objects.get(oi))
                .and_then(|o| o.faces.get(fi))
            {
                face_indices.push((li, oi, fi));
                old_uvs.push(face.uvs);
                new_uvs.push(transform(face.uvs));
            }
        }
        let cmd = commands::ManipulateUVs { faces: face_indices, old_uvs, new_uvs };
        history.push(Box::new(cmd), scene, device);
    }

    fn apply_merge_vertices(
        edit_state: &EditState,
        scene: &mut Scene,
        history: &mut History,
        device: &wgpu::Device,
    ) {
        if !edit_state.selection.vertices.is_empty() {
            // Vertex mode: merge all to centroid
            let mut sum = glam::Vec3::ZERO;
            let mut count = 0;
            for &(li, oi, fi, vi) in &edit_state.selection.vertices {
                if let Some(pos) = scene.layers.get(li)
                    .and_then(|l| l.objects.get(oi))
                    .and_then(|o| o.faces.get(fi))
                    .map(|f| f.positions[vi])
                {
                    sum += pos;
                    count += 1;
                }
            }
            if count > 0 {
                let centroid = sum / count as f32;
                let mut moves = Vec::new();
                for &(li, oi, fi, vi) in &edit_state.selection.vertices {
                    if let Some(old_pos) = scene.layers.get(li)
                        .and_then(|l| l.objects.get(oi))
                        .and_then(|o| o.faces.get(fi))
                        .map(|f| f.positions[vi])
                    {
                        moves.push((li, oi, fi, vi, old_pos, centroid));
                    }
                }
                if !moves.is_empty() {
                    let cmd = commands::MergeVertices { moves };
                    history.push(Box::new(cmd), scene, device);
                }
            }
        } else if !edit_state.selection.faces.is_empty() {
            // Face mode: weld coincident vertices across selected faces
            let threshold = 0.001_f32;
            let threshold_sq = threshold * threshold;
            let mut moves = Vec::new();

            // Collect all vertex positions from selected faces
            let mut verts: Vec<(usize, usize, usize, usize, glam::Vec3)> = Vec::new();
            for &(li, oi, fi) in &edit_state.selection.faces {
                if let Some(face) = scene.layers.get(li)
                    .and_then(|l| l.objects.get(oi))
                    .and_then(|o| o.faces.get(fi))
                {
                    for vi in 0..4 {
                        verts.push((li, oi, fi, vi, face.positions[vi]));
                    }
                }
            }

            // Find coincident pairs and merge to midpoint
            let mut merged = vec![false; verts.len()];
            for i in 0..verts.len() {
                if merged[i] { continue; }
                for j in (i + 1)..verts.len() {
                    if merged[j] { continue; }
                    if verts[i].4.distance_squared(verts[j].4) < threshold_sq {
                        let mid = (verts[i].4 + verts[j].4) * 0.5;
                        if (verts[i].4 - mid).length_squared() > 1e-12 {
                            moves.push((verts[i].0, verts[i].1, verts[i].2, verts[i].3, verts[i].4, mid));
                        }
                        if (verts[j].4 - mid).length_squared() > 1e-12 {
                            moves.push((verts[j].0, verts[j].1, verts[j].2, verts[j].3, verts[j].4, mid));
                        }
                        merged[j] = true;
                    }
                }
            }

            if !moves.is_empty() {
                let cmd = commands::MergeVertices { moves };
                history.push(Box::new(cmd), scene, device);
            }
        }
    }

    fn apply_mirror(
        edit_state: &EditState,
        scene: &mut Scene,
        history: &mut History,
        device: &wgpu::Device,
        axis: usize, // 0=X, 1=Y, 2=Z
    ) {
        let crosshair = scene.crosshair_pos;
        let mut faces_to_mirror = Vec::new();
        let mut tileset_index = None;

        for &(li, oi, fi) in &edit_state.selection.faces {
            if let Some(face) = scene.layers.get(li)
                .and_then(|l| l.objects.get(oi))
                .and_then(|o| o.faces.get(fi))
            {
                faces_to_mirror.push(face.clone());
                if tileset_index.is_none() {
                    tileset_index = scene.layers.get(li)
                        .and_then(|l| l.objects.get(oi))
                        .and_then(|o| o.tileset_index);
                }
            }
        }
        for &(li, oi) in &edit_state.selection.objects {
            if let Some(obj) = scene.layers.get(li).and_then(|l| l.objects.get(oi)) {
                for face in &obj.faces {
                    faces_to_mirror.push(face.clone());
                }
                if tileset_index.is_none() {
                    tileset_index = obj.tileset_index;
                }
            }
        }

        if faces_to_mirror.is_empty() { return; }

        // Mirror each face
        let mirrored: Vec<Face> = faces_to_mirror.iter().map(|face| {
            let mut new_face = face.clone();
            for pos in &mut new_face.positions {
                match axis {
                    0 => pos.x = 2.0 * crosshair.x - pos.x,
                    1 => pos.y = 2.0 * crosshair.y - pos.y,
                    _ => pos.z = 2.0 * crosshair.z - pos.z,
                }
            }
            // Reverse winding to fix normals
            new_face.positions.swap(1, 3);
            new_face.uvs.swap(1, 3);
            new_face.colors.swap(1, 3);
            new_face
        }).collect();

        let layer_idx = scene.active_layer;
        let (object_idx, create_object) = crate::tools::draw::find_target_object(scene, layer_idx, tileset_index);
        let cmd = commands::PlaceTile {
            layer: layer_idx,
            object: object_idx,
            faces: mirrored,
            create_object,
            tileset_index,
        };
        history.push(Box::new(cmd), scene, device);
    }

    /// Apply a translation directly to selected geometry (for live gizmo preview).
    fn apply_translate_live(
        selection: &crate::tools::edit::Selection,
        scene: &mut Scene,
        delta: glam::Vec3,
        _device: &wgpu::Device,
    ) {
        let mut rebuild = std::collections::HashSet::new();
        for &(li, oi, fi) in &selection.faces {
            for pos in &mut scene.layers[li].objects[oi].faces[fi].positions {
                *pos += delta;
            }
            rebuild.insert((li, oi));
        }
        for &(li, oi) in &selection.objects {
            for face in &mut scene.layers[li].objects[oi].faces {
                for pos in &mut face.positions {
                    *pos += delta;
                }
            }
            rebuild.insert((li, oi));
        }
        for &(li, oi, fi, vi) in &selection.vertices {
            scene.layers[li].objects[oi].faces[fi].positions[vi] += delta;
            rebuild.insert((li, oi));
        }
        // Instance transforms: translate instance position (no GPU mesh rebuild needed)
        for &(li, oi, ii) in &selection.instances {
            if let Some(inst) = scene.layers.get_mut(li)
                .and_then(|l| l.objects.get_mut(oi))
                .and_then(|o| o.instances.get_mut(ii))
            {
                inst.position += delta;
            }
        }
        for (li, oi) in rebuild {
            scene.layers[li].objects[oi].rebuild_gpu_mesh(_device);
        }
    }

    /// Apply a rotation directly to selected geometry (for live gizmo preview).
    fn apply_rotate_live(
        selection: &crate::tools::edit::Selection,
        scene: &mut Scene,
        axis: glam::Vec3,
        angle: f32,
        center: glam::Vec3,
        device: &wgpu::Device,
    ) {
        let quat = glam::Quat::from_axis_angle(axis, angle);
        let mut rebuild = std::collections::HashSet::new();
        for &(li, oi, fi) in &selection.faces {
            for pos in &mut scene.layers[li].objects[oi].faces[fi].positions {
                *pos = quat * (*pos - center) + center;
            }
            rebuild.insert((li, oi));
        }
        for &(li, oi) in &selection.objects {
            for face in &mut scene.layers[li].objects[oi].faces {
                for pos in &mut face.positions {
                    *pos = quat * (*pos - center) + center;
                }
            }
            rebuild.insert((li, oi));
        }
        for &(li, oi, fi, vi) in &selection.vertices {
            let pos = &mut scene.layers[li].objects[oi].faces[fi].positions[vi];
            *pos = quat * (*pos - center) + center;
            rebuild.insert((li, oi));
        }
        // Instance transforms: rotate position around center and accumulate rotation
        for &(li, oi, ii) in &selection.instances {
            if let Some(inst) = scene.layers.get_mut(li)
                .and_then(|l| l.objects.get_mut(oi))
                .and_then(|o| o.instances.get_mut(ii))
            {
                inst.position = quat * (inst.position - center) + center;
                inst.rotation = quat * inst.rotation;
            }
        }
        for (li, oi) in rebuild {
            scene.layers[li].objects[oi].rebuild_gpu_mesh(device);
        }
    }

    /// Apply a scale directly to selected geometry (for live gizmo preview).
    fn apply_scale_live(
        selection: &crate::tools::edit::Selection,
        scene: &mut Scene,
        factor: glam::Vec3,
        center: glam::Vec3,
        device: &wgpu::Device,
    ) {
        let mut rebuild = std::collections::HashSet::new();
        for &(li, oi, fi) in &selection.faces {
            for pos in &mut scene.layers[li].objects[oi].faces[fi].positions {
                *pos = center + (*pos - center) * factor;
            }
            rebuild.insert((li, oi));
        }
        for &(li, oi) in &selection.objects {
            for face in &mut scene.layers[li].objects[oi].faces {
                for pos in &mut face.positions {
                    *pos = center + (*pos - center) * factor;
                }
            }
            rebuild.insert((li, oi));
        }
        for &(li, oi, fi, vi) in &selection.vertices {
            let pos = &mut scene.layers[li].objects[oi].faces[fi].positions[vi];
            *pos = center + (*pos - center) * factor;
            rebuild.insert((li, oi));
        }
        // Instance transforms: scale position relative to center and accumulate scale
        for &(li, oi, ii) in &selection.instances {
            if let Some(inst) = scene.layers.get_mut(li)
                .and_then(|l| l.objects.get_mut(oi))
                .and_then(|o| o.instances.get_mut(ii))
            {
                inst.position = center + (inst.position - center) * factor;
                inst.scale *= factor;
            }
        }
        for (li, oi) in rebuild {
            scene.layers[li].objects[oi].rebuild_gpu_mesh(device);
        }
    }
}

/// (start_world_position, vertex_drag_targets) for initiating a vertex drag.
type DragTargets = Option<(glam::Vec3, Vec<(usize, usize, usize, usize, glam::Vec3)>)>;

/// Find a selected vertex near the mouse cursor for vertex drag initiation.
/// Returns (start_world_position, vertex_targets) or None if nothing close enough.
fn find_vertex_drag_targets(
    selected_verts: &[(usize, usize, usize, usize)],
    scene: &Scene,
    mouse_pos: glam::Vec2,
    view_proj: glam::Mat4,
    screen_size: glam::Vec2,
    threshold: f32,
) -> DragTargets {
    let mut best_dist = threshold;
    let mut best_world = glam::Vec3::ZERO;

    // Find the closest selected vertex to the mouse in screen space
    for &(li, oi, fi, vi) in selected_verts {
        if let Some(face) = scene.layers.get(li)
            .and_then(|l| l.objects.get(oi))
            .and_then(|o| o.faces.get(fi))
        {
            let pos = face.positions[vi];
            if let Some(sp) = picking::project_to_screen(pos, view_proj, screen_size) {
                let d = sp.distance(mouse_pos);
                if d < best_dist {
                    best_dist = d;
                    best_world = pos;
                }
            }
        }
    }

    if best_dist >= threshold {
        return None;
    }

    // Collect all selected vertices as drag targets
    let targets: Vec<_> = selected_verts.iter()
        .filter_map(|&(li, oi, fi, vi)| {
            scene.layers.get(li)
                .and_then(|l| l.objects.get(oi))
                .and_then(|o| o.faces.get(fi))
                .map(|face| (li, oi, fi, vi, face.positions[vi]))
        })
        .collect();

    if targets.is_empty() { None } else { Some((best_world, targets)) }
}

/// Find a selected edge near the mouse cursor for edge drag initiation.
/// Returns (start_world_position, vertex_targets_for_both_edge_endpoints) or None.
fn find_edge_drag_targets(
    selected_edges: &[(usize, usize, usize, usize)],
    scene: &Scene,
    mouse_pos: glam::Vec2,
    view_proj: glam::Mat4,
    screen_size: glam::Vec2,
    threshold: f32,
) -> DragTargets {
    let mut best_dist = threshold;
    let mut best_midpoint = glam::Vec3::ZERO;

    // Find the closest selected edge midpoint to the mouse
    for &(li, oi, fi, ei) in selected_edges {
        if let Some(face) = scene.layers.get(li)
            .and_then(|l| l.objects.get(oi))
            .and_then(|o| o.faces.get(fi))
        {
            let a = face.positions[ei];
            let b = face.positions[(ei + 1) % 4];
            let mid = (a + b) * 0.5;
            if let Some(sp) = picking::project_to_screen(mid, view_proj, screen_size) {
                let d = sp.distance(mouse_pos);
                if d < best_dist {
                    best_dist = d;
                    best_midpoint = mid;
                }
            }
        }
    }

    if best_dist >= threshold {
        return None;
    }

    // Collect both endpoints of all selected edges as drag targets (deduplicated)
    let mut targets = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for &(li, oi, fi, ei) in selected_edges {
        if let Some(face) = scene.layers.get(li)
            .and_then(|l| l.objects.get(oi))
            .and_then(|o| o.faces.get(fi))
        {
            let vi_a = ei;
            let vi_b = (ei + 1) % 4;
            if seen.insert((li, oi, fi, vi_a)) {
                targets.push((li, oi, fi, vi_a, face.positions[vi_a]));
            }
            if seen.insert((li, oi, fi, vi_b)) {
                targets.push((li, oi, fi, vi_b, face.positions[vi_b]));
            }
        }
    }

    if targets.is_empty() { None } else { Some((best_midpoint, targets)) }
}

/// Find a selected face under the mouse ray for face drag initiation.
/// Returns (hit_world_position, all_vertices_of_selected_faces) or None.
fn find_face_drag_targets(
    selected_faces: &[(usize, usize, usize)],
    scene: &Scene,
    ray: &Ray,
) -> DragTargets {
    // Raycast to find which selected face was clicked
    let hit = picking::pick_face(ray, scene)?;
    let hit_key = (hit.layer_index, hit.object_index, hit.face_index);

    // Only start drag if the hit face is in the selection
    if !selected_faces.contains(&hit_key) {
        return None;
    }

    // Collect all vertices of all selected faces as drag targets
    let mut targets = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for &(li, oi, fi) in selected_faces {
        if let Some(face) = scene.layers.get(li)
            .and_then(|l| l.objects.get(oi))
            .and_then(|o| o.faces.get(fi))
        {
            for vi in 0..4 {
                if seen.insert((li, oi, fi, vi)) {
                    targets.push((li, oi, fi, vi, face.positions[vi]));
                }
            }
        }
    }

    if targets.is_empty() { None } else { Some((hit.position, targets)) }
}

/// Returns `~/subdir` as a `PathBuf`.
fn dirs_or_home(subdir: &str) -> std::path::PathBuf {
    let home = std::env::var("HOME")
        .unwrap_or_else(|_| ".".to_string());
    std::path::PathBuf::from(home).join(subdir)
}

/// Collect all selected vertex positions. Returns (li, oi, fi, vi, old_pos) for each vertex.
fn collect_selected_verts(
    scene: &crate::scene::Scene,
    sel: &crate::tools::edit::Selection,
) -> Vec<(usize, usize, usize, usize, glam::Vec3)> {
    let mut verts = Vec::new();
    let mut seen = std::collections::HashSet::new();

    // Explicitly selected vertices
    for &(li, oi, fi, vi) in &sel.vertices {
        if seen.insert((li, oi, fi, vi))
            && let Some(face) = scene.layers.get(li)
                .and_then(|l| l.objects.get(oi))
                .and_then(|o| o.faces.get(fi))
        {
            verts.push((li, oi, fi, vi, face.positions[vi]));
        }
    }

    // All vertices of selected faces
    for &(li, oi, fi) in &sel.faces {
        if let Some(face) = scene.layers.get(li)
            .and_then(|l| l.objects.get(oi))
            .and_then(|o| o.faces.get(fi))
        {
            for vi in 0..4 {
                if seen.insert((li, oi, fi, vi)) {
                    verts.push((li, oi, fi, vi, face.positions[vi]));
                }
            }
        }
    }

    // All vertices of selected objects
    for &(li, oi) in &sel.objects {
        if let Some(obj) = scene.layers.get(li).and_then(|l| l.objects.get(oi)) {
            for (fi, face) in obj.faces.iter().enumerate() {
                for vi in 0..4 {
                    if seen.insert((li, oi, fi, vi)) {
                        verts.push((li, oi, fi, vi, face.positions[vi]));
                    }
                }
            }
        }
    }

    verts
}

/// Compute push/pull moves: each vertex moves along the average normal of its faces.
fn compute_push_pull_moves(
    scene: &crate::scene::Scene,
    sel: &crate::tools::edit::Selection,
    distance: f32,
) -> Vec<(usize, usize, usize, usize, glam::Vec3, glam::Vec3)> {
    let verts = collect_selected_verts(scene, sel);
    if verts.is_empty() { return Vec::new(); }

    // For each vertex, compute the average normal of all faces it belongs to within its object
    let mut moves = Vec::new();
    for &(li, oi, fi, vi, old_pos) in &verts {
        // Compute normal of the face this vertex belongs to
        let normal = if let Some(face) = scene.layers.get(li)
            .and_then(|l| l.objects.get(oi))
            .and_then(|o| o.faces.get(fi))
        {
            face.normal()
        } else {
            continue;
        };

        if normal.length_squared() < 1e-8 { continue; }
        let new_pos = old_pos + normal.normalize() * distance;
        moves.push((li, oi, fi, vi, old_pos, new_pos));
    }

    moves
}

/// Compute center moves: align all selected verts to `value` on the given axis (0=X, 1=Y, 2=Z).
fn compute_center_moves(
    scene: &crate::scene::Scene,
    sel: &crate::tools::edit::Selection,
    axis: usize,
    value: f32,
) -> Vec<(usize, usize, usize, usize, glam::Vec3, glam::Vec3)> {
    let verts = collect_selected_verts(scene, sel);
    let mut moves = Vec::new();
    for &(li, oi, fi, vi, old_pos) in &verts {
        let mut new_pos = old_pos;
        match axis {
            0 => new_pos.x = value,
            1 => new_pos.y = value,
            2 => new_pos.z = value,
            _ => {}
        }
        if (new_pos - old_pos).length_squared() > 1e-10 {
            moves.push((li, oi, fi, vi, old_pos, new_pos));
        }
    }
    moves
}

/// Compute straighten moves: project all selected verts onto their best-fit plane.
fn compute_straighten_moves(
    scene: &crate::scene::Scene,
    sel: &crate::tools::edit::Selection,
) -> Vec<(usize, usize, usize, usize, glam::Vec3, glam::Vec3)> {
    let verts = collect_selected_verts(scene, sel);
    if verts.len() < 3 { return Vec::new(); }

    // Compute centroid
    let centroid: glam::Vec3 = verts.iter().map(|v| v.4).sum::<glam::Vec3>() / verts.len() as f32;

    // Compute best-fit normal via covariance matrix eigenvector (simplified: use face normals)
    // For simplicity, use the average face normal of selected faces as the plane normal
    let mut avg_normal = glam::Vec3::ZERO;
    let mut seen_faces = std::collections::HashSet::new();
    for &(li, oi, fi, _, _) in &verts {
        if seen_faces.insert((li, oi, fi))
            && let Some(face) = scene.layers.get(li)
                .and_then(|l| l.objects.get(oi))
                .and_then(|o| o.faces.get(fi))
        {
            avg_normal += face.normal();
        }
    }

    if avg_normal.length_squared() < 1e-8 { return Vec::new(); }
    let plane_normal = avg_normal.normalize();

    // Project each vertex onto the plane defined by (centroid, plane_normal)
    let mut moves = Vec::new();
    for &(li, oi, fi, vi, old_pos) in &verts {
        let offset = old_pos - centroid;
        let dist_to_plane = offset.dot(plane_normal);
        let new_pos = old_pos - plane_normal * dist_to_plane;
        if (new_pos - old_pos).length_squared() > 1e-10 {
            moves.push((li, oi, fi, vi, old_pos, new_pos));
        }
    }

    moves
}

/// Compute the axis-aligned bounding box of a set of faces.
fn compute_faces_aabb(faces: &[Face]) -> (glam::Vec3, glam::Vec3) {
    let mut min = glam::Vec3::splat(f32::MAX);
    let mut max = glam::Vec3::splat(f32::MIN);
    for face in faces {
        for p in &face.positions {
            min = min.min(*p);
            max = max.max(*p);
        }
    }
    (min, max)
}

/// Check if a face is a degenerate quad (triangle) — any 2 vertices are coincident.
fn is_degenerate_quad(face: &Face) -> bool {
    let eps = 1e-5;
    for i in 0..4 {
        for j in (i + 1)..4 {
            if face.positions[i].distance(face.positions[j]) < eps {
                return true;
            }
        }
    }
    false
}

/// Find pairs of selected triangular faces that share an edge and can be merged.
fn find_triangle_merge_pairs(
    scene: &crate::scene::Scene,
    selected_faces: &[(usize, usize, usize)],
) -> Vec<((usize, usize, usize), (usize, usize, usize))> {
    let eps = 1e-4;

    // Filter to only degenerate quads (triangles) and collect their unique verts
    let mut tris: Vec<((usize, usize, usize), [glam::Vec3; 3])> = Vec::new();
    for &(li, oi, fi) in selected_faces {
        if let Some(face) = scene.layers.get(li)
            .and_then(|l| l.objects.get(oi))
            .and_then(|o| o.faces.get(fi))
        {
            if !is_degenerate_quad(face) { continue; }
            // Extract 3 unique vertices
            let mut verts = Vec::with_capacity(3);
            for p in &face.positions {
                if !verts.iter().any(|v: &glam::Vec3| v.distance(*p) < eps) {
                    verts.push(*p);
                }
            }
            if verts.len() == 3 {
                tris.push(((li, oi, fi), [verts[0], verts[1], verts[2]]));
            }
        }
    }

    let mut pairs = Vec::new();
    let mut used = vec![false; tris.len()];

    for i in 0..tris.len() {
        if used[i] { continue; }
        for j in (i + 1)..tris.len() {
            if used[j] { continue; }
            // Must be in the same object
            if tris[i].0.0 != tris[j].0.0 || tris[i].0.1 != tris[j].0.1 { continue; }

            // Count shared vertices
            let mut shared = 0;
            for va in &tris[i].1 {
                for vb in &tris[j].1 {
                    if va.distance(*vb) < eps {
                        shared += 1;
                    }
                }
            }
            if shared == 2 {
                pairs.push((tris[i].0, tris[j].0));
                used[i] = true;
                used[j] = true;
                break;
            }
        }
    }

    pairs
}

/// Auto-flatten UVs for faces affected by a selection transform.
fn auto_flatten_selection_uvs(
    scene: &mut crate::scene::Scene,
    faces: &[(usize, usize, usize)],
    objects: &[(usize, usize)],
    vertices: &[(usize, usize, usize, usize)],
    device: &wgpu::Device,
) {
    let mut affected: std::collections::HashSet<(usize, usize, usize)> = std::collections::HashSet::new();

    for &(li, oi, fi) in faces {
        affected.insert((li, oi, fi));
    }
    for &(li, oi) in objects {
        if let Some(obj) = scene.layers.get(li).and_then(|l| l.objects.get(oi)) {
            for fi in 0..obj.faces.len() {
                affected.insert((li, oi, fi));
            }
        }
    }
    for &(li, oi, fi, _vi) in vertices {
        affected.insert((li, oi, fi));
    }

    let mut rebuild: std::collections::HashSet<(usize, usize)> = std::collections::HashSet::new();
    for (li, oi, fi) in affected {
        if let Some(face) = scene.layers.get_mut(li)
            .and_then(|l| l.objects.get_mut(oi))
            .and_then(|o| o.faces.get_mut(fi))
        {
            face.flatten_uvs();
            rebuild.insert((li, oi));
        }
    }
    for (li, oi) in rebuild {
        if let Some(obj) = scene.layers.get_mut(li).and_then(|l| l.objects.get_mut(oi)) {
            obj.rebuild_gpu_mesh(device);
        }
    }
}
