use glam::{Mat4, Vec3};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Projection {
    Perspective,
    Orthographic,
}

pub struct Camera {
    pub position: Vec3,
    pub target: Vec3,
    pub up: Vec3,
    pub projection: Projection,
    pub fov_y: f32,
    pub ortho_scale: f32,
    pub near: f32,
    pub far: f32,
    aspect: f32,

    // Orbit parameters
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
}

impl Camera {
    pub fn new() -> Self {
        let distance = 10.0;
        let yaw: f32 = -45.0_f32.to_radians();
        let pitch: f32 = 30.0_f32.to_radians();

        let position = Self::orbit_position(Vec3::ZERO, yaw, pitch, distance);

        Self {
            position,
            target: Vec3::ZERO,
            up: Vec3::Y,
            projection: Projection::Perspective,
            fov_y: 45.0_f32.to_radians(),
            ortho_scale: 5.0,
            near: 0.1,
            far: 1000.0,
            aspect: 16.0 / 9.0,
            yaw,
            pitch,
            distance,
        }
    }

    pub fn set_aspect(&mut self, width: f32, height: f32) {
        if height > 0.0 {
            self.aspect = width / height;
        }
    }

    pub fn view_matrix(&self) -> Mat4 {
        Mat4::look_at_rh(self.position, self.target, self.up)
    }

    pub fn projection_matrix(&self) -> Mat4 {
        match self.projection {
            Projection::Perspective => {
                Mat4::perspective_rh(self.fov_y, self.aspect, self.near, self.far)
            }
            Projection::Orthographic => {
                let half_w = self.ortho_scale * self.aspect;
                let half_h = self.ortho_scale;
                Mat4::orthographic_rh(-half_w, half_w, -half_h, half_h, self.near, self.far)
            }
        }
    }

    pub fn view_projection(&self) -> Mat4 {
        self.projection_matrix() * self.view_matrix()
    }

    /// Orbit around the target by yaw/pitch deltas (in radians).
    pub fn orbit(&mut self, delta_yaw: f32, delta_pitch: f32) {
        self.yaw += delta_yaw;
        self.pitch = (self.pitch + delta_pitch).clamp(-89.0_f32.to_radians(), 89.0_f32.to_radians());
        self.update_position();
    }

    /// Zoom by adjusting distance (perspective) or ortho_scale (orthographic).
    pub fn zoom(&mut self, delta: f32) {
        match self.projection {
            Projection::Perspective => {
                self.distance = (self.distance - delta).max(0.5);
                self.update_position();
            }
            Projection::Orthographic => {
                self.ortho_scale = (self.ortho_scale - delta * 0.5).max(0.5);
            }
        }
    }

    /// Pan the target (and camera) in the camera's local XY plane.
    pub fn pan(&mut self, delta_x: f32, delta_y: f32) {
        let forward = (self.target - self.position).normalize();
        let right = forward.cross(self.up).normalize();
        let cam_up = right.cross(forward).normalize();

        let offset = right * delta_x + cam_up * delta_y;
        self.target += offset;
        self.update_position();
    }

    pub fn toggle_projection(&mut self) {
        self.projection = match self.projection {
            Projection::Perspective => Projection::Orthographic,
            Projection::Orthographic => Projection::Perspective,
        };
    }

    fn update_position(&mut self) {
        self.position = Self::orbit_position(self.target, self.yaw, self.pitch, self.distance);
    }

    fn orbit_position(target: Vec3, yaw: f32, pitch: f32, distance: f32) -> Vec3 {
        let x = distance * pitch.cos() * yaw.sin();
        let y = distance * pitch.sin();
        let z = distance * pitch.cos() * yaw.cos();
        target + Vec3::new(x, y, z)
    }
}
