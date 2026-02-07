use glam::Vec2;

/// A tileset texture divided into a grid of tiles.
pub struct Tileset {
    pub name: String,
    pub image_width: u32,
    pub image_height: u32,
    pub tile_width: u32,
    pub tile_height: u32,
    pub gpu_texture: Option<wgpu::Texture>,
    pub bind_group: Option<wgpu::BindGroup>,
    /// egui texture ID for displaying in the tileset browser panel.
    pub egui_texture_id: Option<egui::TextureId>,
    /// Raw RGBA pixel data, kept for egui registration.
    pub image_data: Option<Vec<u8>>,
}

impl Tileset {
    pub fn cols(&self) -> u32 {
        if self.tile_width == 0 { return 0; }
        self.image_width / self.tile_width
    }

    pub fn rows(&self) -> u32 {
        if self.tile_height == 0 { return 0; }
        self.image_height / self.tile_height
    }

    /// Compute UV coordinates for a tile at (col, row) in the tileset grid.
    /// Returns [bottom-left, bottom-right, top-right, top-left] UVs.
    pub fn tile_uvs(&self, col: u32, row: u32) -> [Vec2; 4] {
        let u0 = col as f32 * self.tile_width as f32 / self.image_width as f32;
        let v0 = row as f32 * self.tile_height as f32 / self.image_height as f32;
        let u1 = u0 + self.tile_width as f32 / self.image_width as f32;
        let v1 = v0 + self.tile_height as f32 / self.image_height as f32;

        [
            Vec2::new(u0, v1), // bottom-left
            Vec2::new(u1, v1), // bottom-right
            Vec2::new(u1, v0), // top-right
            Vec2::new(u0, v0), // top-left
        ]
    }

    /// Compute UV coordinates spanning a rectangular region of tiles.
    /// (col0, row0) is the top-left tile, (col1, row1) is the bottom-right tile (inclusive).
    pub fn tile_region_uvs(&self, col0: u32, row0: u32, col1: u32, row1: u32) -> [Vec2; 4] {
        let u0 = col0 as f32 * self.tile_width as f32 / self.image_width as f32;
        let v0 = row0 as f32 * self.tile_height as f32 / self.image_height as f32;
        let u1 = (col1 + 1) as f32 * self.tile_width as f32 / self.image_width as f32;
        let v1 = (row1 + 1) as f32 * self.tile_height as f32 / self.image_height as f32;

        [
            Vec2::new(u0, v1), // bottom-left
            Vec2::new(u1, v1), // bottom-right
            Vec2::new(u1, v0), // top-right
            Vec2::new(u0, v0), // top-left
        ]
    }

    /// Load a tileset from an image file path. Creates GPU resources.
    pub fn load(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bind_group_layout: &wgpu::BindGroupLayout,
        path: &std::path::Path,
        tile_width: u32,
        tile_height: u32,
    ) -> Result<Self, String> {
        let img = image::open(path)
            .map_err(|e| format!("Failed to load image: {e}"))?
            .to_rgba8();

        let (w, h) = img.dimensions();

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("tileset_texture"),
            size: wgpu::Extent3d {
                width: w,
                height: h,
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
            &img,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * w),
                rows_per_image: Some(h),
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );

        let view = texture.create_view(&Default::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("tileset_bg"),
            layout: bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        let image_data = img.into_raw();

        Ok(Self {
            name: path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default(),
            image_width: w,
            image_height: h,
            tile_width,
            tile_height,
            gpu_texture: Some(texture),
            bind_group: Some(bind_group),
            egui_texture_id: None,
            image_data: Some(image_data),
        })
    }

    /// Register this tileset's image with the egui renderer for UI display.
    /// Must be called after load() and before the first frame that needs to display the tileset.
    pub fn register_with_egui(
        &mut self,
        egui_renderer: &mut egui_wgpu::Renderer,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) {
        if self.egui_texture_id.is_some() {
            return; // Already registered
        }

        let Some(ref image_data) = self.image_data else { return };

        // Create a separate texture for egui display
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("tileset_egui_texture"),
            size: wgpu::Extent3d {
                width: self.image_width,
                height: self.image_height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // Upload image data
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            image_data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * self.image_width),
                rows_per_image: Some(self.image_height),
            },
            wgpu::Extent3d {
                width: self.image_width,
                height: self.image_height,
                depth_or_array_layers: 1,
            },
        );

        let view = texture.create_view(&Default::default());

        let id = egui_renderer.register_native_texture(
            device,
            &view,
            wgpu::FilterMode::Nearest,
        );

        self.egui_texture_id = Some(id);
    }
}
