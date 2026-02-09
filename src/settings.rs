use serde::{Serialize, Deserialize};

/// All user-configurable settings, persisted to JSON.
#[derive(Default, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct Settings {
    pub camera: CameraSettings,
    pub display: DisplaySettings,
    pub draw: DrawSettings,
    pub edit: EditSettings,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct CameraSettings {
    pub fov_degrees: f32,
    pub near_plane: f32,
    pub far_plane: f32,
    pub orbit_sensitivity: f32,
    pub pan_sensitivity: f32,
    pub freelook_sensitivity: f32,
    pub freelook_speed: f32,
    pub zoom_speed: f32,
    pub invert_orbit_y: bool,
}

impl Default for CameraSettings {
    fn default() -> Self {
        Self {
            fov_degrees: 45.0,
            near_plane: 0.1,
            far_plane: 1000.0,
            orbit_sensitivity: 0.005,
            pan_sensitivity: 0.01,
            freelook_sensitivity: 0.005,
            freelook_speed: 0.1,
            zoom_speed: 1.0,
            invert_orbit_y: false,
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct DisplaySettings {
    pub bg_color: [f32; 3],
    pub grid_color: [f32; 4],
    pub wireframe_color: [f32; 4],
    pub selection_color: [f32; 4],
    pub vertex_color: [f32; 4],
    pub edge_color: [f32; 4],
    pub hover_color: [f32; 4],
    pub preview_color: [f32; 4],
    pub vertex_size: f32,
    pub undo_limit: usize,
}

impl Default for DisplaySettings {
    fn default() -> Self {
        Self {
            bg_color: [0.15, 0.15, 0.18],
            grid_color: [0.35, 0.35, 0.35, 1.0],
            wireframe_color: [0.8, 0.8, 0.8, 1.0],
            selection_color: [1.0, 1.0, 0.3, 1.0],
            vertex_color: [0.3, 1.0, 1.0, 1.0],
            edge_color: [1.0, 0.6, 0.2, 1.0],
            hover_color: [0.5, 0.7, 1.0, 1.0],
            preview_color: [0.3, 1.0, 0.5, 1.0],
            vertex_size: 0.15,
            undo_limit: 100,
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct DrawSettings {
    pub default_paint_color: [f32; 4],
    pub default_paint_radius: f32,
    pub default_paint_opacity: f32,
}

impl Default for DrawSettings {
    fn default() -> Self {
        Self {
            default_paint_color: [1.0, 0.0, 0.0, 1.0],
            default_paint_radius: 0.0,
            default_paint_opacity: 1.0,
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct EditSettings {
    pub vertex_pick_threshold: f32,
    pub merge_distance: f32,
    pub auto_flatten_uvs: bool,
}

impl Default for EditSettings {
    fn default() -> Self {
        Self {
            vertex_pick_threshold: 12.0,
            merge_distance: 0.001,
            auto_flatten_uvs: false,
        }
    }
}

impl Settings {
    /// Load settings from config file. Falls back to defaults on error.
    pub fn load() -> Self {
        let path = config_path();
        if path.exists()
            && let Ok(data) = std::fs::read_to_string(&path)
            && let Ok(settings) = serde_json::from_str::<Settings>(&data)
        {
            return settings;
        }
        Self::default()
    }

    /// Save settings to config file.
    pub fn save(&self) {
        let path = config_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(data) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(&path, data);
        }
    }
}

fn config_path() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    std::path::PathBuf::from(home).join(".config/cracktile3d/settings.json")
}

/// Which tab is currently active in the settings dialog.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SettingsTab {
    Camera,
    Display,
    Draw,
    Edit,
}
