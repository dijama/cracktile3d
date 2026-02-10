# Cracktile 3D

A tile-based 3D modeling application built in Rust. Place tiles from 2D pixel-art tilesets as textured quads in 3D space to build models and environments — a bridge between 2D pixel-art editors and 3D modeling tools, targeting retro and low-poly game asset creation.

Inspired by [Crocotile 3D](https://crocotile3d.com/), rebuilt from first principles in Rust for native performance.

## Features

### Draw Mode
- **Tile** — Click to place a textured quad on the grid or adjacent to existing faces; hold and drag to paint continuously
- **Sticky** — Extend tiles from the closest edge of an existing face
- **Block** — Place a 6-face cube in one click; subtract mode erases faces inside the block volume
- **Primitive** — Place box, cylinder, cone, sphere, or wedge shapes
- **Vertex Color** — Paint per-vertex colors with configurable brush radius and opacity
- **Prefab** — Create and place reusable object instances
- **Camera-based placement plane** — Camera angle auto-selects XZ/XY/YZ plane; look from the front to build walls, look down to place floors
- **Tilebrush transforms** — Rotate (R/Shift+R) and flip (F/G) tiles before placement
- **Rectangle fill** — Shift+drag with Tile tool to fill a rectangular area
- **Placement plane indicator** — Shows current plane (Top/Front/Side) in tools panel

### Edit Mode
- **Selection levels** — Object, face, vertex, edge
- **3D transform gizmo** — Visual translate/rotate/scale handles with click-drag interaction
- **Transform** — Translate (arrow keys or gizmo), rotate, scale, with grid snapping (Shift=fine, Ctrl=coarse)
- **Operations** — Flip normals, extrude, retile, subdivide, delete, merge vertices
- **Triangle operations** — Divide quads into triangles, merge adjacent triangles back to quads
- **Vertex alignment** — Push/pull along normals, center to axis, straighten vertices
- **UV manipulation** — Rotate CW/CCW, flip horizontal/vertical; floating UV editor panel (Ctrl+U)
- **Auto-flatten UVs** — Optionally recompute UVs proportionally when vertices are moved
- **Geometry** — Mirror X/Y/Z across crosshair plane
- **Edge operations** — Split edge (quad to 2 quads), collapse edge (merge to midpoint)
- **Advanced selection** — Select by normal, overlapping, tilebrush, edge loop, faces from vertices
- **Instances** — Create lightweight copies (Ctrl+Shift+I) that share source geometry with independent transforms; deconstruct back to independent objects
- **Selection tools** — Click, shift-click, marquee drag, select all, invert, select connected
- **Copy/paste** — Ctrl+C/V with crosshair-relative placement
- **Hide/show** — H to hide selected, Shift+H to show all (undoable)

### Tileset Management
- Load PNG tilesets with configurable tile size
- Tileset scales to fill panel width — tiles are large and easy to click regardless of native resolution
- Multi-tile selection by dragging on the tileset
- Zoom 25%-800% with scroll wheel, +/- buttons, or Fit button
- Filled yellow highlight on selected tile region
- Eyedropper (Alt+right-click) to pick tile from existing faces

### Camera
- Orbit (Space+drag or middle mouse), pan (Space+right-drag or Shift+middle)
- Scroll to zoom, numpad presets (front/back/left/right/top/bottom)
- Freelook mode (hold right-click + WASD in Edit mode)
- Perspective/orthographic toggle (Numpad 5)
- 5 camera bookmarks (Ctrl+Shift+1-5 to save, Ctrl+1-5 to recall)
- Configurable FOV, sensitivity, zoom speed, invert Y axis

### File I/O
- Native binary format (.ct3d) with save/load
- Import from Wavefront OBJ (.obj), glTF Binary (.glb), and COLLADA (.dae)
- Export to Wavefront OBJ (.obj), glTF Binary (.glb), glTF (.gltf), and COLLADA (.dae)
- Instances flattened to independent geometry on export
- Screenshot to PNG (F12)
- Recent files menu (remembers last 10 files)

### UI
- Tile placement preview (green wireframe ghost)
- Hover highlight in Edit mode (blue wireframe)
- Edge selection highlight (orange)
- 3D ViewCube for quick camera orientation
- Viewport rulers with world-space coordinate labels
- Floating tileset panel (pop-out/dock with Ctrl+Shift+T)
- Floating UV editor panel (Ctrl+U)
- Clickable edit operation buttons (UV, geometry, edge ops)
- Object + instance tree in layers panel
- Editable vertex positions, UVs, and colors in properties panel (with undo support)
- Elevated grid at crosshair height
- Unsaved changes indicator
- Background color picker
- Wireframe toggle (Z)
- Backface culling toggle
- Lighting preview toggle
- Skybox (gradient or equirectangular panorama)

### Customization
- Customizable keybindings (Edit > Keybindings...) with JSON persistence
- Settings/preferences system (Edit > Preferences...) with per-category tabs
- Configurable colors for grid, wireframe, selection, vertices, edges, hover, preview
- Camera sensitivity, FOV, near/far plane, invert Y axis
- Auto-flatten UVs on vertex edit (optional)

## Controls

### General
| Key | Action |
|-----|--------|
| Tab | Toggle Draw/Edit mode |
| Ctrl+Z / Ctrl+Y | Undo / Redo |
| Ctrl+S | Save |
| Ctrl+O | Open |
| Ctrl+N | New scene |
| F12 | Screenshot |
| Z | Toggle wireframe |
| [ / ] | Decrease / increase grid size |
| Ctrl+U | Toggle UV editor panel |
| Ctrl+Shift+T | Toggle floating tileset panel |
| Numpad 5 | Toggle perspective/orthographic |
| Numpad 1/3/7 | Front / Right / Top view |
| Ctrl+Numpad 1/3/7 | Back / Left / Bottom view |

### Draw Mode
| Key | Action |
|-----|--------|
| 1-5 | Select draw tool |
| WASD | Move crosshair on XZ plane |
| Q / E | Move crosshair down / up |
| R / Shift+R | Rotate tilebrush CW / CCW |
| F | Flip tilebrush vertically |
| G | Flip tilebrush horizontally |
| Left click | Place tile |
| Left click + drag | Paint tiles continuously (Tile tool) |
| Shift+click + drag | Rectangle fill (Tile tool) |
| Right click | Erase tile |
| Alt+Right click | Eyedropper (pick tile from face) |

### Edit Mode
| Key | Action |
|-----|--------|
| Arrow keys | Translate selection (grid step) |
| Shift+Arrow keys | Translate selection (fine: half grid step) |
| Ctrl+Arrow keys | Translate selection (coarse: double grid step) |
| R / Shift+R | Rotate CW / CCW |
| +/- | Scale (in Scale mode) |
| F | Flip normals |
| E | Extrude faces |
| T | Retile selected faces |
| M | Merge vertices |
| C | Center camera on selection |
| H / Shift+H | Hide selected / Show all (undoable) |
| Del / Backspace | Delete selection |
| Ctrl+A | Select all |
| Ctrl+D | Deselect all |
| Ctrl+I | Invert selection |
| Ctrl+L | Select connected |
| Ctrl+C / Ctrl+V | Copy / Paste |
| Alt+D | Subdivide faces |
| Enter | Create object from selection |
| Ctrl+Shift+I | Create instance from selected object |
| T / R / Y | Gizmo: Translate / Rotate / Scale |
| 1 / 2 / 3 / 4 | Selection: Object / Face / Edge / Vertex |
| Ctrl+Shift+1-5 | Save camera bookmark |
| Ctrl+1-5 | Recall camera bookmark |
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
docker run --rm -v "$PWD":/project -w /project rust:1.92 sh -c \
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
│   ├── camera.rs        # Orbit/freelook camera, bookmarks
│   ├── grid.rs          # XZ grid + crosshair + elevated grid
│   ├── gizmo.rs         # 3D transform gizmo (translate/rotate/scale)
│   ├── skybox.rs        # Gradient + equirect panorama skybox
│   └── shaders/         # WGSL shaders (tile, line, skybox)
├── scene/
│   ├── mod.rs           # Scene, Layer structs
│   ├── object.rs        # Object with GPU mesh batching + instances
│   └── mesh.rs          # Face (quad) geometry, tangent_basis, flatten_uvs
├── tools/
│   ├── draw/            # Draw tools, placement, tilebrush, primitives
│   └── edit/            # Selection, transforms, marquee
├── ui/
│   ├── mod.rs           # Menu bar, status bar, settings/keybindings dialogs
│   ├── tools_panel.rs   # Draw/Edit tools (left panel)
│   ├── tileset_panel.rs # Tileset browser (bottom/floating)
│   ├── layers_panel.rs  # Layers + object tree (right panel)
│   ├── properties_panel.rs # Face properties editor
│   ├── uv_panel.rs      # Floating UV editor
│   ├── viewcube.rs      # 3D orientation cube overlay
│   └── rulers.rs        # Viewport rulers with coordinate labels
├── history/             # Undo/redo command pattern (35+ command types)
├── tile/                # Tileset loading, UV computation
├── io/                  # Save/load (.ct3d), import/export (OBJ, GLB, glTF, DAE)
├── keybindings.rs       # Customizable keybinding system
├── settings.rs          # Persistent user preferences
└── util/                # Raycasting, picking, screen projection
```

## License

This project is not yet licensed. All rights reserved.
