use wgpu::util::DeviceExt;
use crate::render::Vertex;
use crate::scene::mesh::Face;

/// A collection of tile faces that share a single draw call.
pub struct Object {
    pub name: String,
    pub faces: Vec<Face>,
    pub gpu_mesh: Option<GpuMesh>,
    pub tileset_bind_group: Option<wgpu::BindGroup>,
}

pub struct GpuMesh {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub index_count: u32,
}

impl Object {
    pub fn new(name: String) -> Self {
        Self {
            name,
            faces: Vec::new(),
            gpu_mesh: None,
            tileset_bind_group: None,
        }
    }

    /// Rebuild GPU buffers from CPU face data.
    pub fn rebuild_gpu_mesh(&mut self, device: &wgpu::Device) {
        if self.faces.is_empty() {
            self.gpu_mesh = None;
            return;
        }

        let mut vertices = Vec::with_capacity(self.faces.len() * 4);
        let mut indices = Vec::with_capacity(self.faces.len() * 6);

        for face in &self.faces {
            let base = vertices.len() as u32;
            vertices.extend_from_slice(&face.vertices());
            indices.extend_from_slice(&Face::indices(base));
        }

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("object_vb"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("object_ib"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        self.gpu_mesh = Some(GpuMesh {
            vertex_buffer,
            index_buffer,
            index_count: indices.len() as u32,
        });
    }
}
