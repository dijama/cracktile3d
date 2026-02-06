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
}

impl Tileset {
    pub fn cols(&self) -> u32 {
        self.image_width / self.tile_width
    }

    pub fn rows(&self) -> u32 {
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
        })
    }
}
