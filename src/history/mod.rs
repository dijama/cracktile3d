/// Undo/redo history using the command pattern.
pub struct History {
    undo_stack: Vec<Box<dyn Command>>,
    redo_stack: Vec<Box<dyn Command>>,
    max_depth: usize,
}

pub trait Command {
    fn execute(&mut self);
    fn undo(&mut self);
    fn description(&self) -> &str;
}

impl History {
    pub fn new() -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            max_depth: 100,
        }
    }

    pub fn push(&mut self, cmd: Box<dyn Command>) {
        self.undo_stack.push(cmd);
        self.redo_stack.clear();
        if self.undo_stack.len() > self.max_depth {
            self.undo_stack.remove(0);
        }
    }

    pub fn undo(&mut self) {
        if let Some(mut cmd) = self.undo_stack.pop() {
            cmd.undo();
            self.redo_stack.push(cmd);
        }
    }

    pub fn redo(&mut self) {
        if let Some(mut cmd) = self.redo_stack.pop() {
            cmd.execute();
            self.undo_stack.push(cmd);
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }
}
