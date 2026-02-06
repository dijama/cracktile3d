# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**Cracktile 3D** is a high-performance, modular Rust reimplementation of [Crocotile 3D](https://crocotile3d.com/) — a tile-based 3D modeling application where users build 3D models and environments by selecting tiles from 2D pixel-art tilesets and placing them as textured quads in 3D space. Think of it as a bridge between 2D pixel-art editors and 3D modeling tools, targeting retro/low-poly game asset creation.

The original Crocotile 3D is built on NW.js + Three.js (JavaScript). This project rebuilds it from first principles in Rust for native performance, using wgpu for GPU rendering.

## Build & Run Commands

```bash
cargo build                    # Debug build
cargo build --release          # Release build
cargo run                      # Run debug
cargo run --release            # Run release
cargo test                     # Run all tests
cargo test <test_name>         # Run a single test
cargo test -- --nocapture      # Run tests with stdout visible
cargo clippy                   # Lint
cargo fmt                      # Format code
cargo fmt -- --check           # Check formatting without modifying
```

## Target Architecture

### Rendering: wgpu
- Use [wgpu](https://crates.io/crates/wgpu) for cross-platform GPU rendering (Vulkan/Metal/DX12/WebGPU)
- Nearest-neighbor texture filtering (`FilterMode::Nearest`) everywhere — pixel art must stay crisp
- Tileset textures loaded as GPU texture atlases; UVs computed from tile grid position
- Batch geometry per-object into single `BufferGeometry` equivalents for minimal draw calls
- Support both perspective and orthographic camera projection

### Windowing & Input: winit + egui
- [winit](https://crates.io/crates/winit) for cross-platform windowing and raw input events
- [egui](https://crates.io/crates/egui) (via egui-wgpu) for immediate-mode UI panels (tileset browser, properties, layers, timeline)
- The 3D viewport is rendered directly via wgpu; egui panels surround it

### Dependency Versions (pinned)
- wgpu 25, egui/egui-wgpu/egui-winit 0.32, winit 0.30, glam 0.29
- egui-wgpu 0.32 uses wgpu internally — our direct wgpu dep MUST match (currently both wgpu 25). If upgrading egui, check `cargo tree` for version conflicts.
- egui-wgpu's `Renderer::render()` requires `&mut RenderPass<'static>` but `begin_render_pass` returns a borrowing lifetime. We use `unsafe { std::mem::transmute }` for the lifetime cast (see `app.rs` redraw). The pass is scoped correctly — this is safe.
- Dev profile has `opt-level = 1` and dependencies at `opt-level = 3` for acceptable debug performance.

### Core Crate Layout

```
src/
├── main.rs                    # Entry point, winit event loop
├── app.rs                     # App struct (ApplicationHandler), GpuState, redraw loop
├── render/
│   ├── mod.rs                 # Re-exports Renderer, Camera, Vertex, etc.
│   ├── renderer.rs            # wgpu init, pipelines (tile + line), render_scene()
│   ├── camera.rs              # Camera with orbit/pan/zoom, perspective + orthographic
│   ├── grid.rs                # GridRenderer — XZ grid lines + crosshair
│   ├── vertex.rs              # Vertex (pos/uv/color) and LineVertex (pos/color) with wgpu layouts
│   └── shaders/
│       ├── tile.wgsl          # Textured quad shader (camera uniform + tileset texture)
│       └── line.wgsl          # Colored line shader (camera uniform only)
├── scene/
│   ├── mod.rs                 # Scene (layers, crosshair_pos), Layer
│   ├── object.rs              # Object (faces + GPU mesh), GpuMesh, rebuild_gpu_mesh()
│   └── mesh.rs                # Face (quad: 4 positions, 4 UVs, 4 colors), tangent_basis()
├── tile/
│   ├── mod.rs
│   └── tileset.rs             # Tileset: load PNG → GPU texture, tile_uvs() computation
├── tools/
│   ├── mod.rs                 # ToolMode enum (Draw, Edit)
│   ├── draw/mod.rs            # Draw mode tools (to be implemented)
│   └── edit/mod.rs            # Edit mode tools (to be implemented)
├── ui/mod.rs                  # draw_ui(): menu bar, tools panel, layers panel, status bar
├── input/mod.rs               # InputState: mouse, keyboard, scroll tracking
├── history/mod.rs             # History + Command trait for undo/redo
├── io/mod.rs                  # Import/export (stub)
├── anim/mod.rs                # Animation system (stub)
└── util/mod.rs                # Shared utilities (stub)
```

## Key Domain Concepts

### Tile Placement Model
- A **tile** is a rectangular region of a tileset texture (e.g., 16x16 pixels at column 3, row 5)
- Placing a tile creates a **quad** (4 vertices, 2 triangles) in 3D space with UVs pointing at that tile
- UV computation: `u = col * tile_width / texture_width`, `v = row * tile_height / texture_height`
- Tiles within the same object are **batched** into a single vertex/index buffer for one draw call per object
- The 3D crosshair (controlled by WASD) determines the placement plane position; camera orientation determines the plane's normal

### Draw Mode vs Edit Mode
- **Draw mode**: Place/erase tiles. Tools: Tile (grid snap), Sticky (edge snap), Block (minecraft-style), Primitive (shapes), Vertex Color
- **Edit mode**: Select and transform existing geometry. Selection levels: object, face, edge, vertex. Operations: translate, rotate, scale, extrude, split, retile, flip normals, merge vertices
- Mode switch via Tab key; tool selection via number keys

### Adjacency-Based Building
When clicking an existing face in Draw mode, a new tile is placed adjacent to it, offset by one grid unit along the face normal. This is the core mechanic for building 3D structures from tiles.

### Scene Hierarchy
Scene → Layers → Objects → Faces (tile quads). Objects can be instanced as prefabs with nested instances sharing geometry but having independent transforms.

### Coordinate System
Y-up, matching common 3D conventions. Grid rounding is configurable with up to 4 presets.

## Architecture Principles

- **ECS-inspired but not ECS**: Use Rust structs and enums for scene entities rather than a full ECS framework. Keep data-oriented where it matters (vertex buffers, batch rendering) but don't over-abstract.
- **Command pattern for undo/redo**: Every mutating action creates a reversible `Command` that can be pushed onto a history stack. Commands store the minimal delta needed to undo.
- **Immediate-mode UI**: egui redraws every frame. UI state lives in the application model, not in widget trees.
- **GPU batching**: Tiles in the same object share one vertex buffer + one draw call. Rebuilding the buffer on edit is acceptable since tile counts per object are typically low (hundreds to low thousands).
- **Raycasting for picking**: Cast rays from mouse position through the camera into the scene to determine which face/vertex/edge the user is interacting with. This is performance-critical — consider GPU picking for complex scenes.

## File Format

The native project format (`.ct3d`) is a binary format using serde + bincode (or MessagePack). It stores:
- Scene hierarchy (layers, objects, instances)
- Per-object vertex positions, indices, UV coordinates, vertex colors
- Tileset references (embedded or external file paths)
- Animation keyframes and bone data
- Camera and project settings

## Key Differences from Crocotile 3D

| Aspect | Crocotile 3D | Cracktile 3D |
|---|---|---|
| Language | JavaScript (NW.js) | Rust |
| Rendering | Three.js (WebGL) | wgpu (Vulkan/Metal/DX12) |
| UI | Custom HTML/CSS | egui (immediate mode) |
| Performance | Limited by JS/WebGL | Native performance, multi-threaded |
| Project format | JSON (.croc) | Binary (.ct3d) |
