use std::collections::HashMap;
use winit::keyboard::KeyCode;

/// Modifier keys for a keybinding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct Modifiers {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
}

impl Modifiers {
    pub const NONE: Self = Self { ctrl: false, shift: false, alt: false };
    pub const CTRL: Self = Self { ctrl: true, shift: false, alt: false };
    pub const SHIFT: Self = Self { ctrl: false, shift: true, alt: false };
    pub const CTRL_SHIFT: Self = Self { ctrl: true, shift: true, alt: false };
}

impl std::fmt::Display for Modifiers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.ctrl { write!(f, "Ctrl+")?; }
        if self.shift { write!(f, "Shift+")?; }
        if self.alt { write!(f, "Alt+")?; }
        Ok(())
    }
}

/// A key combination: modifiers + key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct KeyCombo {
    pub modifiers: Modifiers,
    #[serde(with = "keycode_serde")]
    pub key: KeyCode,
}

mod keycode_serde {
    use super::*;
    use serde::{Serializer, Deserializer, Deserialize};

    pub fn serialize<S: Serializer>(key: &KeyCode, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(key_name(*key))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<KeyCode, D::Error> {
        let name = String::deserialize(d)?;
        key_from_name(&name).ok_or_else(|| serde::de::Error::custom(format!("unknown key: {name}")))
    }
}

impl std::fmt::Display for KeyCombo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}{}", self.modifiers, key_name(self.key))
    }
}

/// All bindable actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Action {
    Undo,
    Redo,
    NewScene,
    SaveScene,
    OpenScene,
    Screenshot,
    ToggleWireframe,
    ToggleFloatingTileset,
    ToggleUvPanel,
    SelectAll,
    DeselectAll,
    InvertSelection,
    Copy,
    Paste,
    Delete,
    MergeVertices,
    ToolTile,
    ToolSticky,
    ToolBlock,
    ToolPrimitive,
    ToolVertexColor,
    ToolPrefab,
    ToggleMode,
    GridIncrease,
    GridDecrease,
    SelectionObject,
    SelectionFace,
    SelectionEdge,
    SelectionVertex,
    GizmoTranslate,
    GizmoRotate,
    GizmoScale,
    TilebrushRotCW,
    TilebrushRotCCW,
    TilebrushFlipH,
    TilebrushFlipV,
}

/// All actions with their display names, for the editor UI.
pub const ALL_ACTIONS: &[(Action, &str)] = &[
    (Action::Undo, "Undo"),
    (Action::Redo, "Redo"),
    (Action::NewScene, "New Scene"),
    (Action::SaveScene, "Save Scene"),
    (Action::OpenScene, "Open Scene"),
    (Action::Screenshot, "Screenshot"),
    (Action::ToggleWireframe, "Toggle Wireframe"),
    (Action::ToggleFloatingTileset, "Toggle Floating Tileset"),
    (Action::ToggleUvPanel, "Toggle UV Panel"),
    (Action::SelectAll, "Select All"),
    (Action::DeselectAll, "Deselect All"),
    (Action::InvertSelection, "Invert Selection"),
    (Action::Copy, "Copy"),
    (Action::Paste, "Paste"),
    (Action::Delete, "Delete"),
    (Action::MergeVertices, "Merge Vertices"),
    (Action::ToolTile, "Tool: Tile"),
    (Action::ToolSticky, "Tool: Sticky"),
    (Action::ToolBlock, "Tool: Block"),
    (Action::ToolPrimitive, "Tool: Primitive"),
    (Action::ToolVertexColor, "Tool: Vertex Color"),
    (Action::ToolPrefab, "Tool: Prefab"),
    (Action::ToggleMode, "Toggle Draw/Edit"),
    (Action::GridIncrease, "Grid Size Increase"),
    (Action::GridDecrease, "Grid Size Decrease"),
    (Action::SelectionObject, "Selection: Object"),
    (Action::SelectionFace, "Selection: Face"),
    (Action::SelectionEdge, "Selection: Edge"),
    (Action::SelectionVertex, "Selection: Vertex"),
    (Action::GizmoTranslate, "Gizmo: Translate"),
    (Action::GizmoRotate, "Gizmo: Rotate"),
    (Action::GizmoScale, "Gizmo: Scale"),
    (Action::TilebrushRotCW, "Tilebrush: Rotate CW"),
    (Action::TilebrushRotCCW, "Tilebrush: Rotate CCW"),
    (Action::TilebrushFlipH, "Tilebrush: Flip H"),
    (Action::TilebrushFlipV, "Tilebrush: Flip V"),
];

/// Keybinding configuration.
pub struct Keybindings {
    pub bindings: HashMap<Action, KeyCombo>,
}

impl Keybindings {
    pub fn defaults() -> Self {
        let mut b = HashMap::new();
        b.insert(Action::Undo, KeyCombo { modifiers: Modifiers::CTRL, key: KeyCode::KeyZ });
        b.insert(Action::Redo, KeyCombo { modifiers: Modifiers::CTRL, key: KeyCode::KeyY });
        b.insert(Action::NewScene, KeyCombo { modifiers: Modifiers::CTRL, key: KeyCode::KeyN });
        b.insert(Action::SaveScene, KeyCombo { modifiers: Modifiers::CTRL, key: KeyCode::KeyS });
        b.insert(Action::OpenScene, KeyCombo { modifiers: Modifiers::CTRL, key: KeyCode::KeyO });
        b.insert(Action::Screenshot, KeyCombo { modifiers: Modifiers::NONE, key: KeyCode::F12 });
        b.insert(Action::ToggleWireframe, KeyCombo { modifiers: Modifiers::NONE, key: KeyCode::KeyZ });
        b.insert(Action::ToggleFloatingTileset, KeyCombo { modifiers: Modifiers::CTRL_SHIFT, key: KeyCode::KeyT });
        b.insert(Action::ToggleUvPanel, KeyCombo { modifiers: Modifiers::CTRL, key: KeyCode::KeyU });
        b.insert(Action::SelectAll, KeyCombo { modifiers: Modifiers::CTRL, key: KeyCode::KeyA });
        b.insert(Action::DeselectAll, KeyCombo { modifiers: Modifiers::CTRL, key: KeyCode::KeyD });
        b.insert(Action::InvertSelection, KeyCombo { modifiers: Modifiers::CTRL, key: KeyCode::KeyI });
        b.insert(Action::Copy, KeyCombo { modifiers: Modifiers::CTRL, key: KeyCode::KeyC });
        b.insert(Action::Paste, KeyCombo { modifiers: Modifiers::CTRL, key: KeyCode::KeyV });
        b.insert(Action::Delete, KeyCombo { modifiers: Modifiers::NONE, key: KeyCode::Delete });
        b.insert(Action::MergeVertices, KeyCombo { modifiers: Modifiers::NONE, key: KeyCode::KeyM });
        b.insert(Action::ToolTile, KeyCombo { modifiers: Modifiers::NONE, key: KeyCode::Digit1 });
        b.insert(Action::ToolSticky, KeyCombo { modifiers: Modifiers::NONE, key: KeyCode::Digit2 });
        b.insert(Action::ToolBlock, KeyCombo { modifiers: Modifiers::NONE, key: KeyCode::Digit3 });
        b.insert(Action::ToolPrimitive, KeyCombo { modifiers: Modifiers::NONE, key: KeyCode::Digit4 });
        b.insert(Action::ToolVertexColor, KeyCombo { modifiers: Modifiers::NONE, key: KeyCode::Digit5 });
        b.insert(Action::ToolPrefab, KeyCombo { modifiers: Modifiers::NONE, key: KeyCode::Digit6 });
        b.insert(Action::ToggleMode, KeyCombo { modifiers: Modifiers::NONE, key: KeyCode::Tab });
        b.insert(Action::GridIncrease, KeyCombo { modifiers: Modifiers::NONE, key: KeyCode::BracketRight });
        b.insert(Action::GridDecrease, KeyCombo { modifiers: Modifiers::NONE, key: KeyCode::BracketLeft });
        b.insert(Action::SelectionObject, KeyCombo { modifiers: Modifiers::NONE, key: KeyCode::Digit1 });
        b.insert(Action::SelectionFace, KeyCombo { modifiers: Modifiers::NONE, key: KeyCode::Digit2 });
        b.insert(Action::SelectionEdge, KeyCombo { modifiers: Modifiers::NONE, key: KeyCode::Digit3 });
        b.insert(Action::SelectionVertex, KeyCombo { modifiers: Modifiers::NONE, key: KeyCode::Digit4 });
        b.insert(Action::GizmoTranslate, KeyCombo { modifiers: Modifiers::NONE, key: KeyCode::KeyT });
        b.insert(Action::GizmoRotate, KeyCombo { modifiers: Modifiers::NONE, key: KeyCode::KeyR });
        b.insert(Action::GizmoScale, KeyCombo { modifiers: Modifiers::NONE, key: KeyCode::KeyY });
        b.insert(Action::TilebrushRotCW, KeyCombo { modifiers: Modifiers::NONE, key: KeyCode::KeyR });
        b.insert(Action::TilebrushRotCCW, KeyCombo { modifiers: Modifiers::SHIFT, key: KeyCode::KeyR });
        b.insert(Action::TilebrushFlipH, KeyCombo { modifiers: Modifiers::NONE, key: KeyCode::KeyF });
        b.insert(Action::TilebrushFlipV, KeyCombo { modifiers: Modifiers::NONE, key: KeyCode::KeyG });
        Self { bindings: b }
    }

    /// Check if an action's keybinding is triggered given the current input state.
    pub fn is_triggered(&self, action: Action, input: &crate::input::InputState) -> bool {
        let Some(combo) = self.bindings.get(&action) else { return false };

        if !input.key_just_pressed(combo.key) {
            return false;
        }

        let ctrl = input.key_held(KeyCode::ControlLeft) || input.key_held(KeyCode::ControlRight);
        let shift = input.key_held(KeyCode::ShiftLeft) || input.key_held(KeyCode::ShiftRight);
        let alt = input.key_held(KeyCode::AltLeft) || input.key_held(KeyCode::AltRight);

        ctrl == combo.modifiers.ctrl && shift == combo.modifiers.shift && alt == combo.modifiers.alt
    }

    /// Get the display string for an action's keybinding.
    pub fn display(&self, action: Action) -> String {
        self.bindings.get(&action).map_or_else(
            || "Unbound".to_string(),
            |c| c.to_string(),
        )
    }

    /// Load keybindings from config file. Falls back to defaults on error.
    pub fn load() -> Self {
        let path = config_path();
        if path.exists()
            && let Ok(data) = std::fs::read_to_string(&path)
            && let Ok(bindings) = serde_json::from_str::<HashMap<Action, KeyCombo>>(&data)
        {
            return Self { bindings };
        }
        Self::defaults()
    }

    /// Save keybindings to config file.
    pub fn save(&self) {
        let path = config_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(data) = serde_json::to_string_pretty(&self.bindings) {
            let _ = std::fs::write(&path, data);
        }
    }
}

fn config_path() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    std::path::PathBuf::from(home).join(".config/cracktile3d/keybindings.json")
}

/// Display name for a key code.
fn key_name(key: KeyCode) -> &'static str {
    match key {
        KeyCode::KeyA => "A",
        KeyCode::KeyB => "B",
        KeyCode::KeyC => "C",
        KeyCode::KeyD => "D",
        KeyCode::KeyE => "E",
        KeyCode::KeyF => "F",
        KeyCode::KeyG => "G",
        KeyCode::KeyH => "H",
        KeyCode::KeyI => "I",
        KeyCode::KeyJ => "J",
        KeyCode::KeyK => "K",
        KeyCode::KeyL => "L",
        KeyCode::KeyM => "M",
        KeyCode::KeyN => "N",
        KeyCode::KeyO => "O",
        KeyCode::KeyP => "P",
        KeyCode::KeyQ => "Q",
        KeyCode::KeyR => "R",
        KeyCode::KeyS => "S",
        KeyCode::KeyT => "T",
        KeyCode::KeyU => "U",
        KeyCode::KeyV => "V",
        KeyCode::KeyW => "W",
        KeyCode::KeyX => "X",
        KeyCode::KeyY => "Y",
        KeyCode::KeyZ => "Z",
        KeyCode::Digit0 => "0",
        KeyCode::Digit1 => "1",
        KeyCode::Digit2 => "2",
        KeyCode::Digit3 => "3",
        KeyCode::Digit4 => "4",
        KeyCode::Digit5 => "5",
        KeyCode::Digit6 => "6",
        KeyCode::Digit7 => "7",
        KeyCode::Digit8 => "8",
        KeyCode::Digit9 => "9",
        KeyCode::F1 => "F1",
        KeyCode::F2 => "F2",
        KeyCode::F3 => "F3",
        KeyCode::F4 => "F4",
        KeyCode::F5 => "F5",
        KeyCode::F6 => "F6",
        KeyCode::F7 => "F7",
        KeyCode::F8 => "F8",
        KeyCode::F9 => "F9",
        KeyCode::F10 => "F10",
        KeyCode::F11 => "F11",
        KeyCode::F12 => "F12",
        KeyCode::Tab => "Tab",
        KeyCode::Delete => "Delete",
        KeyCode::Backspace => "Backspace",
        KeyCode::Enter => "Enter",
        KeyCode::Escape => "Escape",
        KeyCode::Space => "Space",
        KeyCode::BracketLeft => "[",
        KeyCode::BracketRight => "]",
        _ => "?",
    }
}

/// Reverse lookup: display name → KeyCode.
fn key_from_name(name: &str) -> Option<KeyCode> {
    match name {
        "A" => Some(KeyCode::KeyA),
        "B" => Some(KeyCode::KeyB),
        "C" => Some(KeyCode::KeyC),
        "D" => Some(KeyCode::KeyD),
        "E" => Some(KeyCode::KeyE),
        "F" => Some(KeyCode::KeyF),
        "G" => Some(KeyCode::KeyG),
        "H" => Some(KeyCode::KeyH),
        "I" => Some(KeyCode::KeyI),
        "J" => Some(KeyCode::KeyJ),
        "K" => Some(KeyCode::KeyK),
        "L" => Some(KeyCode::KeyL),
        "M" => Some(KeyCode::KeyM),
        "N" => Some(KeyCode::KeyN),
        "O" => Some(KeyCode::KeyO),
        "P" => Some(KeyCode::KeyP),
        "Q" => Some(KeyCode::KeyQ),
        "R" => Some(KeyCode::KeyR),
        "S" => Some(KeyCode::KeyS),
        "T" => Some(KeyCode::KeyT),
        "U" => Some(KeyCode::KeyU),
        "V" => Some(KeyCode::KeyV),
        "W" => Some(KeyCode::KeyW),
        "X" => Some(KeyCode::KeyX),
        "Y" => Some(KeyCode::KeyY),
        "Z" => Some(KeyCode::KeyZ),
        "0" => Some(KeyCode::Digit0),
        "1" => Some(KeyCode::Digit1),
        "2" => Some(KeyCode::Digit2),
        "3" => Some(KeyCode::Digit3),
        "4" => Some(KeyCode::Digit4),
        "5" => Some(KeyCode::Digit5),
        "6" => Some(KeyCode::Digit6),
        "7" => Some(KeyCode::Digit7),
        "8" => Some(KeyCode::Digit8),
        "9" => Some(KeyCode::Digit9),
        "F1" => Some(KeyCode::F1),
        "F2" => Some(KeyCode::F2),
        "F3" => Some(KeyCode::F3),
        "F4" => Some(KeyCode::F4),
        "F5" => Some(KeyCode::F5),
        "F6" => Some(KeyCode::F6),
        "F7" => Some(KeyCode::F7),
        "F8" => Some(KeyCode::F8),
        "F9" => Some(KeyCode::F9),
        "F10" => Some(KeyCode::F10),
        "F11" => Some(KeyCode::F11),
        "F12" => Some(KeyCode::F12),
        "Tab" => Some(KeyCode::Tab),
        "Delete" => Some(KeyCode::Delete),
        "Backspace" => Some(KeyCode::Backspace),
        "Enter" => Some(KeyCode::Enter),
        "Escape" => Some(KeyCode::Escape),
        "Space" => Some(KeyCode::Space),
        "[" => Some(KeyCode::BracketLeft),
        "]" => Some(KeyCode::BracketRight),
        _ => None,
    }
}
