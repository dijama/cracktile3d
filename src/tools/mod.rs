pub mod draw;
pub mod edit;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolMode {
    Draw,
    Edit,
}
