//! Paint tool for in-app tileset editing.

/// Available paint tools.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaintTool {
    Pencil,
    Eraser,
    Eyedropper,
    Bucket,
}

/// State for the paint editor.
pub struct PaintState {
    /// Whether the paint editor window is open.
    pub open: bool,
    /// Index of the tileset being edited (in scene.tilesets[]).
    pub tileset_index: Option<usize>,
    /// Working pixel buffer (RGBA, width * height * 4 bytes).
    pub pixels: Vec<u8>,
    pub width: u32,
    pub height: u32,
    /// Active tool.
    pub tool: PaintTool,
    /// Primary color (RGBA).
    pub primary_color: [u8; 4],
    /// Secondary color (RGBA).
    pub secondary_color: [u8; 4],
    /// Brush size in pixels (1 = single pixel).
    pub brush_size: u32,
    /// Zoom level.
    pub zoom: f32,
    /// Undo stack (full canvas snapshots).
    undo_stack: Vec<Vec<u8>>,
    /// Redo stack.
    redo_stack: Vec<Vec<u8>>,
    /// Whether pixels have been modified since last GPU sync.
    pub dirty: bool,
    /// Whether we are currently in a stroke (mouse held down).
    in_stroke: bool,
}

impl PaintState {
    pub fn new() -> Self {
        Self {
            open: false,
            tileset_index: None,
            pixels: Vec::new(),
            width: 0,
            height: 0,
            tool: PaintTool::Pencil,
            primary_color: [0, 0, 0, 255],
            secondary_color: [255, 255, 255, 255],
            brush_size: 1,
            zoom: 4.0,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            dirty: false,
            in_stroke: false,
        }
    }

    /// Load tileset pixel data into the editor.
    pub fn load_tileset(&mut self, index: usize, pixels: Vec<u8>, width: u32, height: u32) {
        self.tileset_index = Some(index);
        self.pixels = pixels;
        self.width = width;
        self.height = height;
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.dirty = false;
        self.in_stroke = false;
    }

    /// Begin a new stroke (save snapshot for undo).
    pub fn begin_stroke(&mut self) {
        if !self.in_stroke {
            self.undo_stack.push(self.pixels.clone());
            self.redo_stack.clear();
            // Cap undo stack at 50 entries
            if self.undo_stack.len() > 50 {
                self.undo_stack.remove(0);
            }
            self.in_stroke = true;
        }
    }

    /// End the current stroke.
    pub fn end_stroke(&mut self) {
        self.in_stroke = false;
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub fn undo(&mut self) {
        if let Some(prev) = self.undo_stack.pop() {
            self.redo_stack.push(std::mem::replace(&mut self.pixels, prev));
            self.dirty = true;
        }
    }

    pub fn redo(&mut self) {
        if let Some(next) = self.redo_stack.pop() {
            self.undo_stack.push(std::mem::replace(&mut self.pixels, next));
            self.dirty = true;
        }
    }

    /// Paint at pixel coordinates (x, y) with the given color and brush size.
    pub fn paint(&mut self, x: i32, y: i32, color: [u8; 4]) {
        let radius = self.brush_size as i32 / 2;
        let w = self.width as i32;
        let h = self.height as i32;

        for dy in -radius..=radius {
            for dx in -radius..=radius {
                // Circle brush shape
                if dx * dx + dy * dy > radius * radius + radius {
                    continue;
                }
                let px = x + dx;
                let py = y + dy;
                if px >= 0 && px < w && py >= 0 && py < h {
                    let idx = ((py as u32 * self.width + px as u32) * 4) as usize;
                    if idx + 3 < self.pixels.len() {
                        self.pixels[idx] = color[0];
                        self.pixels[idx + 1] = color[1];
                        self.pixels[idx + 2] = color[2];
                        self.pixels[idx + 3] = color[3];
                    }
                }
            }
        }
        self.dirty = true;
    }

    /// Erase at pixel coordinates (set to transparent).
    pub fn erase(&mut self, x: i32, y: i32) {
        self.paint(x, y, [0, 0, 0, 0]);
    }

    /// Sample the color at pixel coordinates.
    pub fn sample(&self, x: u32, y: u32) -> [u8; 4] {
        if x >= self.width || y >= self.height {
            return [0, 0, 0, 255];
        }
        let idx = ((y * self.width + x) * 4) as usize;
        if idx + 3 < self.pixels.len() {
            [self.pixels[idx], self.pixels[idx + 1], self.pixels[idx + 2], self.pixels[idx + 3]]
        } else {
            [0, 0, 0, 255]
        }
    }

    /// Flood fill from (x, y) with the primary color.
    pub fn bucket_fill(&mut self, x: u32, y: u32, color: [u8; 4]) {
        if x >= self.width || y >= self.height {
            return;
        }

        let target = self.sample(x, y);
        if target == color {
            return; // Already the same color
        }

        let mut stack = vec![(x as i32, y as i32)];
        let w = self.width as i32;
        let h = self.height as i32;

        while let Some((px, py)) = stack.pop() {
            if px < 0 || px >= w || py < 0 || py >= h {
                continue;
            }
            let idx = ((py as u32 * self.width + px as u32) * 4) as usize;
            if idx + 3 >= self.pixels.len() {
                continue;
            }
            let current = [self.pixels[idx], self.pixels[idx + 1], self.pixels[idx + 2], self.pixels[idx + 3]];
            if current != target {
                continue;
            }

            self.pixels[idx] = color[0];
            self.pixels[idx + 1] = color[1];
            self.pixels[idx + 2] = color[2];
            self.pixels[idx + 3] = color[3];

            stack.push((px + 1, py));
            stack.push((px - 1, py));
            stack.push((px, py + 1));
            stack.push((px, py - 1));
        }

        self.dirty = true;
    }
}
