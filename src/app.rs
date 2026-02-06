use std::sync::Arc;

use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowAttributes, WindowId};

use winit::keyboard::KeyCode;

use crate::render::Renderer;
use crate::input::InputState;
use crate::scene::Scene;
use crate::tools::ToolMode;
use crate::tools::draw::DrawState;
use crate::tools::edit::EditState;
use crate::history::History;
use crate::util::picking::Ray;

/// Top-level application state.
pub struct App {
    gpu: Option<GpuState>,
    scene: Scene,
    input: InputState,
    tool_mode: ToolMode,
    draw_state: DrawState,
    edit_state: EditState,
    history: History,
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

        // Camera orbit (Space + left mouse drag)
        if self.input.space_held() && self.input.left_pressed {
            let sensitivity = 0.005;
            gpu.renderer.camera.orbit(
                -self.input.mouse_delta.x * sensitivity,
                -self.input.mouse_delta.y * sensitivity,
            );
        }

        // Camera pan (Space + right mouse drag)
        if self.input.space_held() && self.input.right_pressed {
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

        // Toggle projection (Numpad 5)
        if self.input.key_just_pressed(KeyCode::Numpad5) {
            gpu.renderer.camera.toggle_projection();
        }

        // WASD crosshair movement (only when Space is NOT held)
        if !self.input.space_held() {
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

        // Mode toggle (Tab)
        if self.input.key_just_pressed(KeyCode::Tab) {
            self.tool_mode = match self.tool_mode {
                ToolMode::Draw => ToolMode::Edit,
                ToolMode::Edit => ToolMode::Draw,
            };
        }

        // Draw mode: left click places tile (when not orbiting with Space)
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
            self.draw_state.place_tile(&mut self.scene, &ray, &gpu.renderer.device);
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
            self.draw_state.erasing = true;
            self.draw_state.place_tile(&mut self.scene, &ray, &gpu.renderer.device);
            self.draw_state.erasing = false;
        }

        // Edit mode: left click selects, shift+click adds to selection
        if self.tool_mode == ToolMode::Edit
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
            let shift = self.input.key_held(KeyCode::ShiftLeft) || self.input.key_held(KeyCode::ShiftRight);
            self.edit_state.handle_click(&ray, &self.scene, shift);
        }

        // Edit mode: G key to translate selected by one grid step
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
                self.edit_state.translate_selection(&mut self.scene, delta, &gpu.renderer.device);
            }
        }

        // Edit mode: Delete/Backspace to delete selection
        if self.tool_mode == ToolMode::Edit
            && (self.input.key_just_pressed(KeyCode::Delete) || self.input.key_just_pressed(KeyCode::Backspace))
        {
            self.edit_state.delete_selection(&mut self.scene, &gpu.renderer.device);
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

        // Run egui
        let raw_input = gpu.egui_state.take_egui_input(&gpu.window);
        let egui_ctx = gpu.egui_state.egui_ctx().clone();
        let full_output = egui_ctx.run(raw_input, |ctx| {
            crate::ui::draw_ui(ctx, &mut self.scene, &mut self.tool_mode);
        });
        gpu.egui_state.handle_platform_output(&gpu.window, full_output.platform_output);

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
                                r: 0.15,
                                g: 0.15,
                                b: 0.18,
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

                gpu.renderer.render_scene(&mut pass, &self.scene, &self.input);
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
}
