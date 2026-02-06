use glam::Vec3;
use crate::render::vertex::LineVertex;

/// Generates grid and crosshair line geometry on the XZ plane.
pub struct GridRenderer {
    pub vertex_buffer: wgpu::Buffer,
    pub vertex_count: u32,
    pub crosshair_buffer: wgpu::Buffer,
    pub crosshair_vertex_count: u32,
}

impl GridRenderer {
    pub fn new(device: &wgpu::Device, half_extent: i32, cell_size: f32) -> Self {
        let vertices = Self::build_grid_vertices(half_extent, cell_size);
        let vertex_count = vertices.len() as u32;
        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("grid_vertex_buffer"),
            size: (std::mem::size_of::<LineVertex>() * vertices.len()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let crosshair_verts = Self::build_crosshair_vertices(Vec3::ZERO, 0.5);
        let crosshair_vertex_count = crosshair_verts.len() as u32;
        let crosshair_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("crosshair_vertex_buffer"),
            size: (std::mem::size_of::<LineVertex>() * crosshair_verts.len()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            vertex_buffer,
            vertex_count,
            crosshair_buffer,
            crosshair_vertex_count,
        }
    }

    pub fn upload(&self, queue: &wgpu::Queue, half_extent: i32, cell_size: f32, crosshair_pos: Vec3) {
        let grid_verts = Self::build_grid_vertices(half_extent, cell_size);
        queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&grid_verts));

        let crosshair_verts = Self::build_crosshair_vertices(crosshair_pos, cell_size * 0.6);
        queue.write_buffer(&self.crosshair_buffer, 0, bytemuck::cast_slice(&crosshair_verts));
    }

    fn build_grid_vertices(half_extent: i32, cell_size: f32) -> Vec<LineVertex> {
        let mut verts = Vec::new();
        let grid_color = [0.35, 0.35, 0.35, 1.0];
        let axis_color_x = [0.7, 0.2, 0.2, 1.0];
        let axis_color_z = [0.2, 0.2, 0.7, 1.0];

        for i in -half_extent..=half_extent {
            let offset = i as f32 * cell_size;
            let extent = half_extent as f32 * cell_size;

            // Line along Z axis (varies X)
            let color = if i == 0 { axis_color_z } else { grid_color };
            verts.push(LineVertex { position: [offset, 0.0, -extent], color });
            verts.push(LineVertex { position: [offset, 0.0, extent], color });

            // Line along X axis (varies Z)
            let color = if i == 0 { axis_color_x } else { grid_color };
            verts.push(LineVertex { position: [-extent, 0.0, offset], color });
            verts.push(LineVertex { position: [extent, 0.0, offset], color });
        }

        verts
    }

    fn build_crosshair_vertices(pos: Vec3, size: f32) -> Vec<LineVertex> {
        let r = [1.0, 0.3, 0.3, 1.0];
        let g = [0.3, 1.0, 0.3, 1.0];
        let b = [0.3, 0.3, 1.0, 1.0];

        vec![
            // X axis (red)
            LineVertex { position: [pos.x - size, pos.y, pos.z], color: r },
            LineVertex { position: [pos.x + size, pos.y, pos.z], color: r },
            // Y axis (green)
            LineVertex { position: [pos.x, pos.y - size, pos.z], color: g },
            LineVertex { position: [pos.x, pos.y + size, pos.z], color: g },
            // Z axis (blue)
            LineVertex { position: [pos.x, pos.y, pos.z - size], color: b },
            LineVertex { position: [pos.x, pos.y, pos.z + size], color: b },
        ]
    }
}
