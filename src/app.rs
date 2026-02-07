use std::sync::Arc;

use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowAttributes, WindowId};

use winit::keyboard::KeyCode;

use crate::render::Renderer;
use crate::render::camera::CameraMode;
use crate::input::InputState;
use crate::scene::mesh::Face;
use crate::scene::{Scene, GRID_PRESETS};
use crate::tools::ToolMode;
use crate::tools::draw::{DrawState, DrawTool};
use crate::tools::edit::{EditState, GizmoMode};
use crate::history::History;
use crate::history::commands;
use crate::ui::UiAction;
use crate::util::picking::Ray;

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
            bg_color: [0.15, 0.15, 0.18],
            last_save_path: None,
            preview_faces: Vec::new(),
            hover_face: None,
            has_unsaved_changes: false,
            confirm_dialog: None,
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

        if gpu.renderer.camera.mode == CameraMode::Freelook {
            // Freelook mouse look
            let sensitivity = 0.005;
            gpu.renderer.camera.freelook_look(
                -self.input.mouse_delta.x * sensitivity,
                self.input.mouse_delta.y * sensitivity,
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
            if orbiting {
                let sensitivity = 0.005;
                gpu.renderer.camera.orbit(
                    -self.input.mouse_delta.x * sensitivity,
                    -self.input.mouse_delta.y * sensitivity,
                );
            }

            // Camera pan (Space + right drag, or Shift + middle mouse drag)
            let panning = (self.input.space_held() && self.input.right_pressed)
                || (self.input.middle_pressed && (self.input.key_held(KeyCode::ShiftLeft) || self.input.key_held(KeyCode::ShiftRight)));
            if panning {
                let sensitivity = 0.01 * gpu.renderer.camera.distance;
                gpu.renderer.camera.pan(
                    -self.input.mouse_delta.x * sensitivity,
                    self.input.mouse_delta.y * sensitivity,
                );
            }

            // Camera zoom (scroll wheel)
            if self.input.scroll_delta != 0.0 {
                gpu.renderer.camera.zoom(self.input.scroll_delta);
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

        // Grid preset cycling ([ / ])
        if self.input.key_just_pressed(KeyCode::BracketRight)
            && self.scene.grid_preset_index + 1 < GRID_PRESETS.len()
        {
            self.scene.grid_preset_index += 1;
            self.scene.grid_cell_size = GRID_PRESETS[self.scene.grid_preset_index];
        }
        if self.input.key_just_pressed(KeyCode::BracketLeft)
            && self.scene.grid_preset_index > 0
        {
            self.scene.grid_preset_index -= 1;
            self.scene.grid_cell_size = GRID_PRESETS[self.scene.grid_preset_index];
        }

        // Wireframe toggle (Z) — only when not Ctrl+Z (undo)
        if self.input.key_just_pressed(KeyCode::KeyZ)
            && !self.input.key_held(KeyCode::ControlLeft)
            && !self.input.key_held(KeyCode::ControlRight)
        {
            self.wireframe = !self.wireframe;
        }

        // Mode toggle (Tab)
        if self.input.key_just_pressed(KeyCode::Tab) {
            self.tool_mode = match self.tool_mode {
                ToolMode::Draw => ToolMode::Edit,
                ToolMode::Edit => ToolMode::Draw,
            };
        }

        // Number keys switch draw tools
        if self.tool_mode == ToolMode::Draw && !self.input.space_held() {
            if self.input.key_just_pressed(KeyCode::Digit1) { self.draw_state.tool = DrawTool::Tile; }
            if self.input.key_just_pressed(KeyCode::Digit2) { self.draw_state.tool = DrawTool::Sticky; }
            if self.input.key_just_pressed(KeyCode::Digit3) { self.draw_state.tool = DrawTool::Block; }
            if self.input.key_just_pressed(KeyCode::Digit4) { self.draw_state.tool = DrawTool::Primitive; }
            if self.input.key_just_pressed(KeyCode::Digit5) { self.draw_state.tool = DrawTool::VertexColor; }
        }

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
                // Vertex color tool: paint hit face
                if let Some(hit) = crate::util::picking::pick_face(&ray, &self.scene) {
                    let c = self.draw_state.paint_color;
                    let new_color = glam::Vec4::new(c[0], c[1], c[2], c[3]);
                    let cmd = commands::PaintVertexColor {
                        targets: vec![(hit.layer_index, hit.object_index, hit.face_index)],
                        new_color,
                        old_colors: Vec::new(),
                    };
                    self.history.push(Box::new(cmd), &mut self.scene, &gpu.renderer.device);
                }
            } else if let Some(result) = self.draw_state.compute_placement(&self.scene, &ray) {
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
            if let Some(hit) = crate::util::picking::pick_face(&ray, &self.scene) {
                let face = &self.scene.layers[hit.layer_index].objects[hit.object_index].faces[hit.face_index];
                // Find which tileset and tile coords match these UVs
                let obj = &self.scene.layers[hit.layer_index].objects[hit.object_index];
                if let Some(ts_idx) = obj.tileset_index {
                    self.scene.active_tileset = Some(ts_idx);
                    if let Some(tileset) = self.scene.tilesets.get(ts_idx) {
                        // Reverse-lookup: find tile col/row from UV
                        let uv = face.uvs[0]; // bottom-left UV
                        let col = (uv.x * tileset.image_width as f32 / tileset.tile_width as f32).floor() as u32;
                        let row = (uv.y * tileset.image_height as f32 / tileset.tile_height as f32).floor() as u32;
                        self.draw_state.selected_tile = (col, row);
                        self.draw_state.selected_tile_end = (col, row);
                    }
                }
            }
        }

        // Edit mode: marquee selection on drag release, or point-click selection
        if self.tool_mode == ToolMode::Edit && !self.input.space_held() {
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

        // Edit mode: translate selection by one grid step
        if self.tool_mode == ToolMode::Edit && !self.edit_state.selection.is_empty() {
            let step = self.scene.grid_cell_size;
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

        // Edit mode: Delete/Backspace to delete selection
        if self.tool_mode == ToolMode::Edit
            && (self.input.key_just_pressed(KeyCode::Delete) || self.input.key_just_pressed(KeyCode::Backspace))
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
            // Snap crosshair to centroid of selected faces
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
            // Collect face indices to hide
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
            for &(li, oi, fi) in &to_hide {
                if let Some(face) = self.scene.layers.get_mut(li)
                    .and_then(|l| l.objects.get_mut(oi))
                    .and_then(|o| o.faces.get_mut(fi))
                {
                    face.hidden = true;
                }
            }
            // Rebuild affected objects
            let rebuild: std::collections::HashSet<(usize, usize)> = to_hide.iter().map(|&(li, oi, _)| (li, oi)).collect();
            for (li, oi) in rebuild {
                self.scene.layers[li].objects[oi].rebuild_gpu_mesh(&gpu.renderer.device);
            }
            self.edit_state.selection.clear();
        }

        // Show all hidden tiles (Shift+H)
        if self.input.key_just_pressed(KeyCode::KeyH) && shift && !self.input.space_held() {
            let mut rebuild: std::collections::HashSet<(usize, usize)> = std::collections::HashSet::new();
            for (li, layer) in self.scene.layers.iter_mut().enumerate() {
                for (oi, obj) in layer.objects.iter_mut().enumerate() {
                    for face in &mut obj.faces {
                        if face.hidden {
                            face.hidden = false;
                            rebuild.insert((li, oi));
                        }
                    }
                }
            }
            for (li, oi) in rebuild {
                self.scene.layers[li].objects[oi].rebuild_gpu_mesh(&gpu.renderer.device);
            }
        }

        // Undo/Redo hotkeys
        let ctrl = self.input.key_held(KeyCode::ControlLeft) || self.input.key_held(KeyCode::ControlRight);
        if ctrl && self.input.key_just_pressed(KeyCode::KeyZ) {
            self.history.undo(&mut self.scene, &gpu.renderer.device);
        }
        if ctrl && self.input.key_just_pressed(KeyCode::KeyY) {
            self.history.redo(&mut self.scene, &gpu.renderer.device);
        }

        // Ctrl+N — New scene (confirm if unsaved)
        if ctrl && self.input.key_just_pressed(KeyCode::KeyN) {
            if self.history.dirty {
                self.confirm_dialog = Some(ConfirmDialog::NewScene);
            } else {
                self.pending_action = Some(UiAction::NewScene);
            }
        }

        if ctrl && self.input.key_just_pressed(KeyCode::KeyS) {
            self.pending_action = Some(UiAction::SaveScene);
        }
        if ctrl && self.input.key_just_pressed(KeyCode::KeyO) {
            self.pending_action = Some(UiAction::OpenScene);
        }

        // Select All (Ctrl+A) / Deselect All (Ctrl+D)
        if ctrl && self.input.key_just_pressed(KeyCode::KeyA) {
            self.edit_state.select_all(&self.scene);
        }
        if ctrl && self.input.key_just_pressed(KeyCode::KeyD) {
            self.edit_state.selection.clear();
        }

        // Invert selection (Ctrl+I)
        if ctrl && self.input.key_just_pressed(KeyCode::KeyI) {
            self.edit_state.invert_selection(&self.scene);
        }

        // Copy (Ctrl+C) — copy selected faces to clipboard
        if ctrl && self.input.key_just_pressed(KeyCode::KeyC) && !self.edit_state.selection.is_empty() {
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

        // Paste (Ctrl+V) — paste clipboard at crosshair position
        if ctrl && self.input.key_just_pressed(KeyCode::KeyV)
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

        // Compute placement preview (every frame in Draw mode)
        self.preview_faces.clear();
        if self.tool_mode == ToolMode::Draw
            && !self.input.space_held()
            && self.draw_state.tool != DrawTool::VertexColor
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
            if let Some(result) = self.draw_state.compute_placement(&self.scene, &ray) {
                self.preview_faces = result.faces;
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
        let mut ui_action = UiAction::None;
        let full_output = egui_ctx.run(raw_input, |ctx| {
            ui_action = crate::ui::draw_ui(
                ctx,
                &mut self.scene,
                &mut self.tool_mode,
                &mut self.draw_state,
                &mut self.edit_state,
                &self.history,
                self.wireframe,
                &mut self.bg_color,
                self.has_unsaved_changes,
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
                            ui_action = UiAction::NewScene;
                        }
                        ConfirmDialog::Quit => {
                            ui_action = UiAction::Quit;
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
                    ui_action = UiAction::ConfirmTilesetLoad;
                }
                if cancelled {
                    pending_tileset = None;
                }
            }
        });

        // Put pending tileset back
        self.pending_tileset = pending_tileset;

        gpu.egui_state.handle_platform_output(&gpu.window, full_output.platform_output);

        // Merge pending keyboard-triggered action with UI action
        let ui_action = if let Some(pending) = self.pending_action.take() {
            pending
        } else {
            ui_action
        };

        // Handle UI actions
        match ui_action {
            UiAction::NewScene => {
                self.scene = Scene::new();
                self.edit_state.selection.clear();
                self.history.clear();
                self.last_save_path = None;
                self.has_unsaved_changes = false;
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
            UiAction::SaveScene => {
                if let Some(ref path) = self.last_save_path {
                    match crate::io::save_scene(&self.scene, path) {
                        Ok(()) => {
                            log::info!("Saved scene to {:?}", path);
                            self.history.mark_saved();
                        }
                        Err(e) => log::error!("Failed to save: {e}"),
                    }
                } else {
                    Self::do_save_scene(&self.scene, &mut self.last_save_path, &mut self.history);
                }
            }
            UiAction::SaveSceneAs => {
                Self::do_save_scene(&self.scene, &mut self.last_save_path, &mut self.history);
            }
            UiAction::OpenScene => {
                Self::do_open_scene(
                    &mut self.scene,
                    &mut self.edit_state,
                    &mut self.history,
                    &gpu.renderer,
                );
            }
            UiAction::ExportObj => {
                Self::do_export_obj(&self.scene);
            }
            UiAction::ExportGlb => {
                Self::do_export_glb(&self.scene);
            }
            UiAction::ToggleWireframe => {
                self.wireframe = !self.wireframe;
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
            UiAction::Quit => {
                std::process::exit(0);
            }
            UiAction::None => {}
        }

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
                gpu.renderer.render_preview(&mut pass, &self.preview_faces);
                gpu.renderer.render_hover(&mut pass, &self.scene, self.hover_face);
                gpu.renderer.render_selection(&mut pass, &self.scene, &self.edit_state.selection);
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
                // egui-wgpu requires 'static but the pass lifetime is actually bounded
                // by this block scope. This is a well-known pattern for egui-wgpu integration.
                let pass_static: &mut wgpu::RenderPass<'static> =
                    unsafe { std::mem::transmute(&mut pass) };
                gpu.egui_renderer.render(pass_static, &paint_jobs, &screen_descriptor);
            }

            gpu.renderer.queue.submit(std::iter::once(encoder.finish()));
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

    fn do_save_scene(scene: &Scene, last_save_path: &mut Option<std::path::PathBuf>, history: &mut History) {
        let file = rfd::FileDialog::new()
            .add_filter("Cracktile 3D", &["ct3d"])
            .set_title("Save Scene")
            .save_file();

        if let Some(path) = file {
            match crate::io::save_scene(scene, &path) {
                Ok(()) => {
                    log::info!("Saved scene to {:?}", path);
                    *last_save_path = Some(path);
                    history.mark_saved();
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
                    log::info!("Opened scene from {:?}", path);
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
}
