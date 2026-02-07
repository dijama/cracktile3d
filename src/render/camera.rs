use glam::{Mat4, Vec3};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Projection {
    Perspective,
    Orthographic,
}

/// Snapshot of camera state for bookmarks.
#[derive(Debug, Clone)]
pub struct CameraBookmark {
    pub position: Vec3,
    pub target: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
    pub projection: Projection,
    pub ortho_scale: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CameraMode {
    Orbit,
    Freelook,
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

    // Freelook
    pub mode: CameraMode,
    pub freelook_speed: f32,
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
            mode: CameraMode::Orbit,
            freelook_speed: 0.1,
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

    /// Front view: looking along -Z
    pub fn set_view_front(&mut self) {
        self.yaw = 0.0;
        self.pitch = 0.0;
        self.update_position();
    }

    /// Back view: looking along +Z
    pub fn set_view_back(&mut self) {
        self.yaw = std::f32::consts::PI;
        self.pitch = 0.0;
        self.update_position();
    }

    /// Right view: looking along -X
    pub fn set_view_right(&mut self) {
        self.yaw = -std::f32::consts::FRAC_PI_2;
        self.pitch = 0.0;
        self.update_position();
    }

    /// Left view: looking along +X
    pub fn set_view_left(&mut self) {
        self.yaw = std::f32::consts::FRAC_PI_2;
        self.pitch = 0.0;
        self.update_position();
    }

    /// Top view: looking down along -Y
    pub fn set_view_top(&mut self) {
        self.yaw = 0.0;
        self.pitch = 89.0_f32.to_radians();
        self.update_position();
    }

    /// Bottom view: looking up along +Y
    pub fn set_view_bottom(&mut self) {
        self.yaw = 0.0;
        self.pitch = -89.0_f32.to_radians();
        self.update_position();
    }

    /// Center the camera orbit on a given target point.
    pub fn center_on(&mut self, target: Vec3) {
        self.target = target;
        self.update_position();
    }

    /// Enter freelook (FPS) camera mode, preserving current position and direction.
    pub fn enter_freelook(&mut self) {
        self.mode = CameraMode::Freelook;
    }

    /// Exit freelook mode, recalculating orbit parameters from current position.
    pub fn exit_freelook(&mut self) {
        self.mode = CameraMode::Orbit;
        let diff = self.position - self.target;
        self.distance = diff.length().max(0.5);
        self.pitch = (diff.y / self.distance).asin();
        self.yaw = diff.x.atan2(diff.z);
    }

    /// Move in freelook mode by camera-relative directions.
    pub fn freelook_move(&mut self, forward: f32, right: f32, up: f32) {
        let dir = (self.target - self.position).normalize();
        let right_vec = dir.cross(self.up).normalize();
        let offset = dir * forward * self.freelook_speed
            + right_vec * right * self.freelook_speed
            + Vec3::Y * up * self.freelook_speed;
        self.position += offset;
        self.target += offset;
    }

    /// Rotate the camera in freelook mode (mouse look).
    pub fn freelook_look(&mut self, dx: f32, dy: f32) {
        self.yaw += dx;
        self.pitch = (self.pitch - dy).clamp(-89.0_f32.to_radians(), 89.0_f32.to_radians());

        // Recompute target from position + direction
        let dir_x = self.pitch.cos() * self.yaw.sin();
        let dir_y = self.pitch.sin();
        let dir_z = self.pitch.cos() * self.yaw.cos();
        // In orbit mode, position = target + offset. In freelook, target = position - offset direction
        // We want target in front of position: target = position - direction_from_target_to_position
        self.target = self.position - Vec3::new(dir_x, dir_y, dir_z) * self.distance;
    }

    /// Save current camera state as a bookmark.
    pub fn to_bookmark(&self) -> CameraBookmark {
        CameraBookmark {
            position: self.position,
            target: self.target,
            yaw: self.yaw,
            pitch: self.pitch,
            distance: self.distance,
            projection: self.projection,
            ortho_scale: self.ortho_scale,
        }
    }

    /// Restore camera state from a bookmark.
    pub fn apply_bookmark(&mut self, bm: &CameraBookmark) {
        self.position = bm.position;
        self.target = bm.target;
        self.yaw = bm.yaw;
        self.pitch = bm.pitch;
        self.distance = bm.distance;
        self.projection = bm.projection;
        self.ortho_scale = bm.ortho_scale;
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
