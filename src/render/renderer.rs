use std::sync::Arc;
use wgpu::util::DeviceExt;

use crate::input::InputState;
use crate::render::camera::Camera;
use crate::render::grid::GridRenderer;
use crate::render::vertex::{LineVertex, Vertex};
use crate::scene::Scene;
use crate::scene::mesh::Face;
use crate::tools::edit::Selection;

const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

pub struct Renderer {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub surface: wgpu::Surface<'static>,
    pub surface_format: wgpu::TextureFormat,
    pub config: wgpu::SurfaceConfiguration,
    pub depth_view: wgpu::TextureView,

    pub camera: Camera,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,

    tile_pipeline: wgpu::RenderPipeline,
    line_pipeline: wgpu::RenderPipeline,
    selection_line_pipeline: wgpu::RenderPipeline,
    grid: GridRenderer,

    // Placeholder 1x1 white texture + bind group for untextured rendering
    placeholder_bind_group: wgpu::BindGroup,

    pub tile_bind_group_layout: wgpu::BindGroupLayout,
}

impl Renderer {
    pub async fn new(window: Arc<winit::window::Window>) -> Self {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        });

        let surface = instance.create_surface(window.clone()).expect("failed to create surface");

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .expect("no suitable GPU adapter found");

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                ..Default::default()
            })
            .await
            .expect("failed to create device");

        let size = window.inner_size();
        let caps = surface.get_capabilities(&adapter);
        let surface_format = caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        let depth_view = Self::create_depth_texture(&device, config.width, config.height);

        // Camera uniform
        let camera = Camera::new();
        let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("camera_uniform"),
            size: 64, // mat4x4<f32>
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let camera_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("camera_bgl"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("camera_bg"),
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });

        // Tile texture bind group layout
        let tile_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("tile_bgl"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        // Tile pipeline
        let tile_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("tile_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/tile.wgsl").into()),
        });

        let tile_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("tile_pipeline_layout"),
            bind_group_layouts: &[&camera_bind_group_layout, &tile_bind_group_layout],
            push_constant_ranges: &[],
        });

        let tile_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("tile_pipeline"),
            layout: Some(&tile_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &tile_shader,
                entry_point: Some("vs_main"),
                buffers: &[Vertex::LAYOUT],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &tile_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None, // Tiles can be viewed from both sides
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });

        // Line pipeline
        let line_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("line_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/line.wgsl").into()),
        });

        let line_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("line_pipeline_layout"),
            bind_group_layouts: &[&camera_bind_group_layout],
            push_constant_ranges: &[],
        });

        let line_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("line_pipeline"),
            layout: Some(&line_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &line_shader,
                entry_point: Some("vs_main"),
                buffers: &[LineVertex::LAYOUT],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &line_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::LineList,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });

        // Selection overlay line pipeline (renders on top via depth bias)
        let selection_line_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("selection_line_pipeline"),
            layout: Some(&line_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &line_shader,
                entry_point: Some("vs_main"),
                buffers: &[LineVertex::LAYOUT],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &line_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::LineList,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: Default::default(),
                bias: wgpu::DepthBiasState {
                    constant: -2,
                    slope_scale: -1.0,
                    clamp: 0.0,
                },
            }),
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });

        let grid = GridRenderer::new(&device, 20, 1.0);

        // Placeholder 1x1 white texture for untextured tiles
        let placeholder_texture = device.create_texture_with_data(
            &queue,
            &wgpu::TextureDescriptor {
                label: Some("placeholder_tex"),
                size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &[255, 255, 255, 255],
        );
        let placeholder_view = placeholder_texture.create_view(&Default::default());
        let placeholder_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let placeholder_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("placeholder_bg"),
            layout: &tile_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&placeholder_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&placeholder_sampler),
                },
            ],
        });

        Self {
            device,
            queue,
            surface,
            surface_format,
            config,
            depth_view,
            camera,
            camera_buffer,
            camera_bind_group,
            tile_pipeline,
            line_pipeline,
            selection_line_pipeline,
            grid,
            placeholder_bind_group,
            tile_bind_group_layout,
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
        self.depth_view = Self::create_depth_texture(&self.device, width, height);
        self.camera.set_aspect(width as f32, height as f32);
    }

    /// Upload per-frame data (camera, grid) before the render pass begins.
    pub fn prepare_frame(&mut self, scene: &Scene) {
        let vp = self.camera.view_projection();
        let vp_raw: [f32; 16] = vp.to_cols_array();
        self.queue.write_buffer(&self.camera_buffer, 0, bytemuck::cast_slice(&vp_raw));
        self.grid.upload(&self.queue, 20, 1.0, scene.crosshair_pos);
    }

    pub fn render_scene<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'a>,
        scene: &Scene,
        _input: &InputState,
        wireframe: bool,
    ) {
        // Draw grid
        pass.set_pipeline(&self.line_pipeline);
        pass.set_bind_group(0, &self.camera_bind_group, &[]);
        pass.set_vertex_buffer(0, self.grid.vertex_buffer.slice(..));
        pass.draw(0..self.grid.vertex_count, 0..1);

        // Draw elevated grid (when crosshair is above/below ground)
        if self.grid.elevated_vertex_count > 0 {
            pass.set_vertex_buffer(0, self.grid.elevated_buffer.slice(..));
            pass.draw(0..self.grid.elevated_vertex_count, 0..1);
        }

        // Draw crosshair
        pass.set_vertex_buffer(0, self.grid.crosshair_buffer.slice(..));
        pass.draw(0..self.grid.crosshair_vertex_count, 0..1);

        if wireframe {
            self.render_wireframe(pass, scene);
        } else {
            // Draw scene objects as solid tiles
            pass.set_pipeline(&self.tile_pipeline);
            pass.set_bind_group(0, &self.camera_bind_group, &[]);

            for layer in &scene.layers {
                if !layer.visible {
                    continue;
                }
                for object in &layer.objects {
                    if let Some(ref gpu_mesh) = object.gpu_mesh {
                        let bind_group = object.tileset_index
                            .and_then(|idx| scene.tilesets.get(idx))
                            .and_then(|ts| ts.bind_group.as_ref())
                            .unwrap_or(&self.placeholder_bind_group);
                        pass.set_bind_group(1, bind_group, &[]);
                        pass.set_vertex_buffer(0, gpu_mesh.vertex_buffer.slice(..));
                        pass.set_index_buffer(gpu_mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                        pass.draw_indexed(0..gpu_mesh.index_count, 0, 0..1);
                    }
                }
            }
        }
    }

    /// Draw all scene geometry as wireframe outlines (gray lines).
    fn render_wireframe<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>, scene: &Scene) {
        let color = [0.8, 0.8, 0.8, 1.0];
        let mut line_verts: Vec<LineVertex> = Vec::new();

        for layer in &scene.layers {
            if !layer.visible {
                continue;
            }
            for object in &layer.objects {
                for face in &object.faces {
                    let p = &face.positions;
                    for i in 0..4 {
                        let a = p[i];
                        let b = p[(i + 1) % 4];
                        line_verts.push(LineVertex { position: a.into(), color });
                        line_verts.push(LineVertex { position: b.into(), color });
                    }
                }
            }
        }

        if line_verts.is_empty() {
            return;
        }

        let buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("wireframe_lines"),
            contents: bytemuck::cast_slice(&line_verts),
            usage: wgpu::BufferUsages::VERTEX,
        });

        pass.set_pipeline(&self.line_pipeline);
        pass.set_bind_group(0, &self.camera_bind_group, &[]);
        pass.set_vertex_buffer(0, buffer.slice(..));
        pass.draw(0..line_verts.len() as u32, 0..1);
    }

    /// Draw wireframe outlines for selected faces/objects/vertices.
    pub fn render_selection<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'a>,
        scene: &Scene,
        selection: &Selection,
    ) {
        if selection.is_empty() {
            return;
        }

        let highlight_color = [1.0, 1.0, 0.3, 1.0]; // Yellow
        let vertex_color = [0.3, 1.0, 1.0, 1.0]; // Cyan
        let mut line_verts: Vec<LineVertex> = Vec::new();

        // Face-level selection: draw quad edges
        for &(li, oi, fi) in &selection.faces {
            if let Some(face) = scene.layers.get(li)
                .and_then(|l| l.objects.get(oi))
                .and_then(|o| o.faces.get(fi))
            {
                let p = &face.positions;
                for i in 0..4 {
                    let a = p[i];
                    let b = p[(i + 1) % 4];
                    line_verts.push(LineVertex { position: a.into(), color: highlight_color });
                    line_verts.push(LineVertex { position: b.into(), color: highlight_color });
                }
            }
        }

        // Object-level selection: outline all faces
        for &(li, oi) in &selection.objects {
            if let Some(object) = scene.layers.get(li).and_then(|l| l.objects.get(oi)) {
                for face in &object.faces {
                    let p = &face.positions;
                    for i in 0..4 {
                        let a = p[i];
                        let b = p[(i + 1) % 4];
                        line_verts.push(LineVertex { position: a.into(), color: highlight_color });
                        line_verts.push(LineVertex { position: b.into(), color: highlight_color });
                    }
                }
            }
        }

        // Edge-level selection: draw highlighted edges
        let edge_color = [1.0, 0.6, 0.2, 1.0]; // Orange
        for &(li, oi, fi, ei) in &selection.edges {
            if let Some(face) = scene.layers.get(li)
                .and_then(|l| l.objects.get(oi))
                .and_then(|o| o.faces.get(fi))
            {
                let a = face.positions[ei];
                let b = face.positions[(ei + 1) % 4];
                line_verts.push(LineVertex { position: a.into(), color: edge_color });
                line_verts.push(LineVertex { position: b.into(), color: edge_color });
            }
        }

        // Vertex-level selection: draw small crosshairs
        for &(li, oi, fi, vi) in &selection.vertices {
            if let Some(pos) = scene.layers.get(li)
                .and_then(|l| l.objects.get(oi))
                .and_then(|o| o.faces.get(fi))
                .map(|f| f.positions[vi])
            {
                let s = 0.15;
                line_verts.push(LineVertex { position: [pos.x - s, pos.y, pos.z], color: vertex_color });
                line_verts.push(LineVertex { position: [pos.x + s, pos.y, pos.z], color: vertex_color });
                line_verts.push(LineVertex { position: [pos.x, pos.y - s, pos.z], color: vertex_color });
                line_verts.push(LineVertex { position: [pos.x, pos.y + s, pos.z], color: vertex_color });
                line_verts.push(LineVertex { position: [pos.x, pos.y, pos.z - s], color: vertex_color });
                line_verts.push(LineVertex { position: [pos.x, pos.y, pos.z + s], color: vertex_color });
            }
        }

        if line_verts.is_empty() {
            return;
        }

        let buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("selection_lines"),
            contents: bytemuck::cast_slice(&line_verts),
            usage: wgpu::BufferUsages::VERTEX,
        });

        pass.set_pipeline(&self.selection_line_pipeline);
        pass.set_bind_group(0, &self.camera_bind_group, &[]);
        pass.set_vertex_buffer(0, buffer.slice(..));
        pass.draw(0..line_verts.len() as u32, 0..1);
    }

    /// Render a placement preview as colored wireframe outlines.
    pub fn render_preview<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'a>,
        faces: &[Face],
    ) {
        if faces.is_empty() {
            return;
        }

        let color = [0.3, 1.0, 0.5, 1.0]; // Green
        let mut line_verts: Vec<LineVertex> = Vec::new();

        for face in faces {
            let p = &face.positions;
            for i in 0..4 {
                let a = p[i];
                let b = p[(i + 1) % 4];
                line_verts.push(LineVertex { position: a.into(), color });
                line_verts.push(LineVertex { position: b.into(), color });
            }
        }

        let buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("preview_lines"),
            contents: bytemuck::cast_slice(&line_verts),
            usage: wgpu::BufferUsages::VERTEX,
        });

        pass.set_pipeline(&self.selection_line_pipeline);
        pass.set_bind_group(0, &self.camera_bind_group, &[]);
        pass.set_vertex_buffer(0, buffer.slice(..));
        pass.draw(0..line_verts.len() as u32, 0..1);
    }

    /// Render a hover highlight on a single face.
    pub fn render_hover<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'a>,
        scene: &Scene,
        hover: Option<(usize, usize, usize)>,
    ) {
        let Some((li, oi, fi)) = hover else { return };
        let Some(face) = scene.layers.get(li)
            .and_then(|l| l.objects.get(oi))
            .and_then(|o| o.faces.get(fi))
        else { return };

        let color = [0.5, 0.7, 1.0, 1.0]; // Light blue
        let mut line_verts: Vec<LineVertex> = Vec::new();
        let p = &face.positions;
        for i in 0..4 {
            let a = p[i];
            let b = p[(i + 1) % 4];
            line_verts.push(LineVertex { position: a.into(), color });
            line_verts.push(LineVertex { position: b.into(), color });
        }

        let buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("hover_lines"),
            contents: bytemuck::cast_slice(&line_verts),
            usage: wgpu::BufferUsages::VERTEX,
        });

        pass.set_pipeline(&self.selection_line_pipeline);
        pass.set_bind_group(0, &self.camera_bind_group, &[]);
        pass.set_vertex_buffer(0, buffer.slice(..));
        pass.draw(0..line_verts.len() as u32, 0..1);
    }

    /// Toggle lighting preview. Currently a no-op placeholder for future shader support.
    pub fn set_lighting_enabled(&mut self, _enabled: bool) {
        // TODO: When lighting shader is implemented, update the camera uniform buffer
        // to include light direction and lighting-enabled flag.
    }

    fn create_depth_texture(device: &wgpu::Device, width: u32, height: u32) -> wgpu::TextureView {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("depth_texture"),
            size: wgpu::Extent3d {
                width: width.max(1),
                height: height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        texture.create_view(&Default::default())
    }
}
