pub mod commands;

use crate::scene::Scene;

/// Undo/redo history using the command pattern.
pub struct History {
    undo_stack: Vec<Box<dyn Command>>,
    redo_stack: Vec<Box<dyn Command>>,
    max_depth: usize,
    /// Set to true on push/undo/redo, cleared by `mark_saved()`.
    pub dirty: bool,
}

pub trait Command {
    fn apply(&mut self, scene: &mut Scene, device: &wgpu::Device);
    fn undo(&mut self, scene: &mut Scene, device: &wgpu::Device);
    fn description(&self) -> &str;
}

impl History {
    pub fn new() -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            max_depth: 100,
            dirty: false,
        }
    }

    pub fn push(&mut self, mut cmd: Box<dyn Command>, scene: &mut Scene, device: &wgpu::Device) {
        cmd.apply(scene, device);
        self.undo_stack.push(cmd);
        self.redo_stack.clear();
        if self.undo_stack.len() > self.max_depth {
            self.undo_stack.remove(0);
        }
        self.dirty = true;
    }

    pub fn undo(&mut self, scene: &mut Scene, device: &wgpu::Device) {
        if let Some(mut cmd) = self.undo_stack.pop() {
            cmd.undo(scene, device);
            self.redo_stack.push(cmd);
            self.dirty = true;
        }
    }

    pub fn redo(&mut self, scene: &mut Scene, device: &wgpu::Device) {
        if let Some(mut cmd) = self.redo_stack.pop() {
            cmd.apply(scene, device);
            self.undo_stack.push(cmd);
            self.dirty = true;
        }
    }

    pub fn mark_saved(&mut self) {
        self.dirty = false;
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub fn undo_len(&self) -> usize {
        self.undo_stack.len()
    }

    pub fn redo_len(&self) -> usize {
        self.redo_stack.len()
    }

    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }
}
