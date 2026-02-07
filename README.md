# Cracktile 3D

A tile-based 3D modeling application built in Rust. Place tiles from 2D pixel-art tilesets as textured quads in 3D space to build models and environments — a bridge between 2D pixel-art editors and 3D modeling tools, targeting retro and low-poly game asset creation.

Inspired by [Crocotile 3D](https://crocotile3d.com/), rebuilt from first principles in Rust for native performance.

## Features

### Draw Mode
- **Tile** — Click to place a textured quad on the grid or adjacent to existing faces
- **Sticky** — Extend tiles from the closest edge of an existing face
- **Block** — Place a 6-face cube in one click
- **Primitive** — Place box, cylinder, cone, sphere, or wedge shapes
- **Vertex Color** — Paint per-vertex colors on faces

### Edit Mode
- **Selection levels** — Object, face, vertex, edge
- **Transform** — Translate (arrow keys), rotate (R), scale (+/-)
- **Operations** — Flip normals, extrude, retile, subdivide, delete
- **Selection tools** — Click, shift-click, marquee drag, select all, invert, select connected
- **Copy/paste** — Ctrl+C/V with crosshair-relative placement
- **Hide/show** — H to hide selected, Shift+H to show all

### Tileset Management
- Load PNG tilesets with configurable tile size
- Multi-tile selection by dragging on the tileset
- Zoom in/out with scroll wheel or +/- buttons
- Eyedropper (Alt+right-click) to pick tile from existing faces

### Camera
- Orbit (Space+drag or middle mouse), pan (Space+right-drag or Shift+middle)
- Scroll to zoom, numpad presets (front/back/left/right/top/bottom)
- Freelook mode (hold right-click + WASD in Edit mode)
- Perspective/orthographic toggle (Numpad 5)

### File I/O
- Native binary format (.ct3d) with save/load
- Export to Wavefront OBJ (.obj)
- Export to glTF Binary (.glb)

### UI
- Tile placement preview (green wireframe ghost)
- Hover highlight in Edit mode (blue wireframe)
- Clickable edit operation buttons
- Object tree in layers panel
- Editable vertex positions, UVs, and colors in properties panel
- Elevated grid at crosshair height
- Unsaved changes indicator
- Background color picker
- Wireframe toggle (Z)

## Controls

### General
| Key | Action |
|-----|--------|
| Tab | Toggle Draw/Edit mode |
| Ctrl+Z / Ctrl+Y | Undo / Redo |
| Ctrl+S | Save |
| Ctrl+O | Open |
| Ctrl+N | New scene |
| Z | Toggle wireframe |
| [ / ] | Decrease / increase grid size |
| Numpad 5 | Toggle perspective/orthographic |
| Numpad 1/3/7 | Front / Right / Top view |
| Ctrl+Numpad 1/3/7 | Back / Left / Bottom view |

### Draw Mode
| Key | Action |
|-----|--------|
| 1-5 | Select draw tool |
| WASD | Move crosshair on XZ plane |
| Q / E | Move crosshair down / up |
| Left click | Place tile |
| Right click | Erase tile |
| Alt+Right click | Eyedropper (pick tile from face) |

### Edit Mode
| Key | Action |
|-----|--------|
| Arrow keys | Translate selection |
| R / Shift+R | Rotate CW / CCW |
| +/- | Scale (in Scale mode) |
| F | Flip normals |
| E | Extrude faces |
| T | Retile selected faces |
| C | Center camera on selection |
| H / Shift+H | Hide selected / Show all |
| Del / Backspace | Delete selection |
| Ctrl+A | Select all |
| Ctrl+D | Deselect all |
| Ctrl+I | Invert selection |
| Ctrl+L | Select connected |
| Ctrl+C / Ctrl+V | Copy / Paste |
| Alt+D | Subdivide faces |
| Enter | Create object from selection |
| Right-click + WASD | Freelook camera |

## Building from Source

### Requirements
- Rust 1.75+ (tested with 1.92)
- A GPU with Vulkan, Metal, or DirectX 12 support

### Build & Run
```bash
cargo run --release
```

### Cross-compile for Windows (from Linux)
```bash
# Requires Docker
docker run --rm -v "$PWD":/app -w /app rust:1.92 bash -c \
  "rustup target add x86_64-pc-windows-gnu && \
   apt-get update -qq && apt-get install -y -qq gcc-mingw-w64-x86-64 > /dev/null 2>&1 && \
   cargo build --release --target x86_64-pc-windows-gnu"
```

## Tech Stack

| Component | Library |
|-----------|---------|
| Rendering | wgpu 25 (Vulkan/Metal/DX12) |
| UI | egui 0.32 (immediate mode) |
| Windowing | winit 0.30 |
| Math | glam 0.29 |
| Serialization | serde + bincode |
| File dialogs | rfd |

## Architecture

```
src/
├── main.rs              # Entry point, winit event loop
├── app.rs               # Application state, input handling, render loop
├── render/
│   ├── renderer.rs      # wgpu pipelines, scene rendering
│   ├── camera.rs        # Orbit/freelook camera
│   ├── grid.rs          # XZ grid + crosshair + elevated grid
│   └── shaders/         # WGSL shaders (tile + line)
├── scene/
│   ├── mod.rs           # Scene, Layer structs
│   ├── object.rs        # Object with GPU mesh batching
│   └── mesh.rs          # Face (quad) geometry
├── tools/
│   ├── draw/            # Draw tools + primitive generators
│   └── edit/            # Edit tools + selection logic
├── ui/                  # egui panels (tools, layers, tileset, properties)
├── history/             # Undo/redo command pattern
├── tile/                # Tileset loading + UV computation
├── io/                  # Save/load (.ct3d), export (OBJ, GLB)
└── util/                # Raycasting, picking
```

## License

This project is not yet licensed. All rights reserved.
