use std::fmt::Write as FmtWrite;
use std::fs;
use std::io::Write as IoWrite;
use std::path::{Path, PathBuf};
use glam::{Vec2, Vec3, Vec4};
use crate::scene::Scene;
use crate::scene::mesh::Face;

/// Magic header bytes for the .ct3d file format.
const MAGIC: &[u8; 4] = b"CT3D";
/// Current file format version.
const VERSION: u32 = 1;

/// Save a scene to a .ct3d file.
pub fn save_scene(scene: &Scene, path: &Path) -> Result<(), String> {
    let payload = bincode::serialize(scene)
        .map_err(|e| format!("Serialization failed: {e}"))?;

    let mut data = Vec::with_capacity(MAGIC.len() + 4 + payload.len());
    data.extend_from_slice(MAGIC);
    data.extend_from_slice(&VERSION.to_le_bytes());
    data.extend_from_slice(&payload);

    fs::write(path, &data)
        .map_err(|e| format!("Write failed: {e}"))?;

    Ok(())
}

/// Load a scene from a .ct3d file.
/// GPU meshes will be None — caller must call rebuild_gpu_mesh() on all objects.
pub fn load_scene(path: &Path) -> Result<Scene, String> {
    let data = fs::read(path)
        .map_err(|e| format!("Read failed: {e}"))?;

    if data.len() < 8 {
        return Err("File too small".to_string());
    }

    if &data[0..4] != MAGIC {
        return Err("Not a Cracktile 3D file (bad magic)".to_string());
    }

    let version = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
    if version > VERSION {
        return Err(format!("File version {version} is newer than supported ({VERSION})"));
    }

    let scene: Scene = bincode::deserialize(&data[8..])
        .map_err(|e| format!("Deserialization failed: {e}"))?;

    Ok(scene)
}

/// Export the scene as a Wavefront .obj file.
pub fn export_obj(scene: &Scene, path: &Path) -> Result<(), String> {
    let mut positions = Vec::new();
    let mut texcoords = Vec::new();
    let mut objects: Vec<(String, Vec<(usize, usize)>)> = Vec::new();

    for layer in &scene.layers {
        if !layer.visible { continue; }
        for object in &layer.objects {
            let mut face_refs = Vec::new();
            for face in &object.faces {
                let base_v = positions.len();
                let base_vt = texcoords.len();
                positions.extend_from_slice(&face.positions);
                texcoords.extend_from_slice(&face.uvs);
                face_refs.push((base_v, base_vt));
            }
            if !face_refs.is_empty() {
                objects.push((object.name.clone(), face_refs));
            }
        }
    }

    let mut out = String::new();
    writeln!(out, "# Exported from Cracktile 3D").unwrap();
    writeln!(out).unwrap();

    for p in &positions {
        writeln!(out, "v {} {} {}", p.x, p.y, p.z).unwrap();
    }
    writeln!(out).unwrap();

    for uv in &texcoords {
        writeln!(out, "vt {} {}", uv.x, uv.y).unwrap();
    }
    writeln!(out).unwrap();

    for (name, face_refs) in &objects {
        writeln!(out, "o {name}").unwrap();
        for &(base_v, base_vt) in face_refs {
            let v = base_v + 1; // OBJ is 1-indexed
            let vt = base_vt + 1;
            writeln!(
                out,
                "f {}/{} {}/{} {}/{} {}/{}",
                v, vt, v + 1, vt + 1, v + 2, vt + 2, v + 3, vt + 3,
            ).unwrap();
        }
    }

    fs::write(path, &out).map_err(|e| format!("Write failed: {e}"))
}

/// Path to the recent files config file.
fn recent_files_path() -> PathBuf {
    if let Ok(home) = std::env::var("HOME") {
        let dir = PathBuf::from(home).join(".config").join("cracktile3d");
        let _ = fs::create_dir_all(&dir);
        dir.join("recent.json")
    } else {
        PathBuf::from("recent.json")
    }
}

/// Load recent files list from config.
pub fn load_recent_files() -> Vec<PathBuf> {
    let path = recent_files_path();
    if let Ok(data) = fs::read_to_string(&path) {
        // Simple JSON array of strings
        let mut files = Vec::new();
        for line in data.lines() {
            let trimmed = line.trim().trim_matches(|c| c == '[' || c == ']' || c == ',');
            let trimmed = trimmed.trim().trim_matches('"');
            if !trimmed.is_empty() {
                let p = PathBuf::from(trimmed);
                if p.exists() {
                    files.push(p);
                }
            }
        }
        files
    } else {
        Vec::new()
    }
}

/// Save recent files list to config.
pub fn save_recent_files(files: &[PathBuf]) {
    let path = recent_files_path();
    let entries: Vec<String> = files.iter()
        .map(|p| format!("  \"{}\"", p.to_string_lossy().replace('\\', "\\\\").replace('"', "\\\"")))
        .collect();
    let json = format!("[\n{}\n]", entries.join(",\n"));
    let _ = fs::write(path, json);
}

/// Import a Wavefront OBJ file. Returns a list of (faces, optional_name) per object.
pub fn import_obj(path: &Path) -> Result<Vec<(Vec<Face>, Option<String>)>, String> {
    let content = fs::read_to_string(path)
        .map_err(|e| format!("Read failed: {e}"))?;

    let mut positions: Vec<Vec3> = Vec::new();
    let mut texcoords: Vec<Vec2> = Vec::new();
    let mut current_name: Option<String> = None;
    let mut current_faces: Vec<Face> = Vec::new();
    let mut objects: Vec<(Vec<Face>, Option<String>)> = Vec::new();

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') { continue; }

        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.is_empty() { continue; }

        match parts[0] {
            "v" if parts.len() >= 4 => {
                let x: f32 = parts[1].parse().unwrap_or(0.0);
                let y: f32 = parts[2].parse().unwrap_or(0.0);
                let z: f32 = parts[3].parse().unwrap_or(0.0);
                positions.push(Vec3::new(x, y, z));
            }
            "vt" if parts.len() >= 3 => {
                let u: f32 = parts[1].parse().unwrap_or(0.0);
                let v: f32 = parts[2].parse().unwrap_or(0.0);
                texcoords.push(Vec2::new(u, v));
            }
            "o" | "g" => {
                if !current_faces.is_empty() {
                    objects.push((std::mem::take(&mut current_faces), current_name.take()));
                }
                current_name = parts.get(1).map(|s| s.to_string());
            }
            "f" if parts.len() >= 4 => {
                // Parse face indices (v/vt/vn format)
                let mut face_verts: Vec<(usize, usize)> = Vec::new();
                for &part in &parts[1..] {
                    let indices: Vec<&str> = part.split('/').collect();
                    let vi: usize = indices[0].parse::<usize>().unwrap_or(1) - 1;
                    let ti: usize = if indices.len() > 1 && !indices[1].is_empty() {
                        indices[1].parse::<usize>().unwrap_or(1) - 1
                    } else {
                        0
                    };
                    face_verts.push((vi, ti));
                }

                // Handle quads directly, triangulate n-gons into quads
                if face_verts.len() == 4 {
                    let get_pos = |i: usize| positions.get(face_verts[i].0).copied().unwrap_or(Vec3::ZERO);
                    let get_uv = |i: usize| texcoords.get(face_verts[i].1).copied().unwrap_or(Vec2::ZERO);
                    current_faces.push(Face {
                        positions: [get_pos(0), get_pos(1), get_pos(2), get_pos(3)],
                        uvs: [get_uv(0), get_uv(1), get_uv(2), get_uv(3)],
                        colors: [Vec4::ONE; 4],
                        hidden: false,
                    });
                } else if face_verts.len() == 3 {
                    // Triangle → degenerate quad (duplicate last vertex)
                    let get_pos = |i: usize| positions.get(face_verts[i].0).copied().unwrap_or(Vec3::ZERO);
                    let get_uv = |i: usize| texcoords.get(face_verts[i].1).copied().unwrap_or(Vec2::ZERO);
                    current_faces.push(Face {
                        positions: [get_pos(0), get_pos(1), get_pos(2), get_pos(2)],
                        uvs: [get_uv(0), get_uv(1), get_uv(2), get_uv(2)],
                        colors: [Vec4::ONE; 4],
                        hidden: false,
                    });
                } else if face_verts.len() > 4 {
                    // Fan triangulate into quads where possible
                    let get_pos = |i: usize| positions.get(face_verts[i].0).copied().unwrap_or(Vec3::ZERO);
                    let get_uv = |i: usize| texcoords.get(face_verts[i].1).copied().unwrap_or(Vec2::ZERO);
                    let mut i = 1;
                    while i + 2 < face_verts.len() {
                        if i + 3 <= face_verts.len() {
                            // Make a quad
                            let i2 = i + 1;
                            let i3 = if i + 2 < face_verts.len() { i + 2 } else { 0 };
                            current_faces.push(Face {
                                positions: [get_pos(0), get_pos(i), get_pos(i2), get_pos(i3)],
                                uvs: [get_uv(0), get_uv(i), get_uv(i2), get_uv(i3)],
                                colors: [Vec4::ONE; 4],
                                hidden: false,
                            });
                            i += 3;
                        } else {
                            // Remaining triangle
                            let i2 = i + 1;
                            current_faces.push(Face {
                                positions: [get_pos(0), get_pos(i), get_pos(i2), get_pos(i2)],
                                uvs: [get_uv(0), get_uv(i), get_uv(i2), get_uv(i2)],
                                colors: [Vec4::ONE; 4],
                                hidden: false,
                            });
                            i += 2;
                        }
                    }
                }
            }
            _ => {}
        }
    }

    // Push last object
    if !current_faces.is_empty() {
        objects.push((current_faces, current_name));
    }

    if objects.is_empty() {
        return Err("No geometry found in OBJ file".to_string());
    }

    Ok(objects)
}

/// Import a GLB (binary glTF 2.0) file. Returns a list of (faces, optional_name) per mesh.
pub fn import_glb(path: &Path) -> Result<Vec<(Vec<Face>, Option<String>)>, String> {
    let data = fs::read(path)
        .map_err(|e| format!("Read failed: {e}"))?;

    if data.len() < 12 {
        return Err("File too small for GLB".to_string());
    }

    // GLB header
    let magic = &data[0..4];
    if magic != b"glTF" {
        return Err("Not a GLB file (bad magic)".to_string());
    }
    let _version = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);

    // Parse chunks
    let mut offset = 12;
    let mut json_data: Option<&[u8]> = None;
    let mut bin_data: Option<&[u8]> = None;

    while offset + 8 <= data.len() {
        let chunk_len = u32::from_le_bytes([data[offset], data[offset+1], data[offset+2], data[offset+3]]) as usize;
        let chunk_type = u32::from_le_bytes([data[offset+4], data[offset+5], data[offset+6], data[offset+7]]);
        offset += 8;

        if offset + chunk_len > data.len() { break; }

        if chunk_type == 0x4E4F534A { // JSON
            json_data = Some(&data[offset..offset + chunk_len]);
        } else if chunk_type == 0x004E4942 { // BIN
            bin_data = Some(&data[offset..offset + chunk_len]);
        }
        offset += chunk_len;
    }

    let json_bytes = json_data.ok_or("No JSON chunk in GLB")?;
    let bin = bin_data.unwrap_or(&[]);
    let json_str = std::str::from_utf8(json_bytes)
        .map_err(|e| format!("Invalid JSON UTF-8: {e}"))?;

    // Minimal JSON parsing for glTF — extract meshes, accessors, bufferViews
    // We use a simple approach: find arrays by key and parse them
    let mut objects: Vec<(Vec<Face>, Option<String>)> = Vec::new();

    // Extract buffer views: [{byteOffset, byteLength}, ...]
    let buffer_views = parse_glb_buffer_views(json_str);
    let accessors = parse_glb_accessors(json_str);
    let meshes = parse_glb_meshes(json_str);

    for mesh in &meshes {
        let mut faces = Vec::new();

        if let (Some(pos_acc), Some(idx_acc)) = (mesh.position_accessor, mesh.indices_accessor) {
            let positions = read_accessor_vec3(&accessors, &buffer_views, bin, pos_acc);
            let texcoords = mesh.texcoord_accessor
                .map(|acc| read_accessor_vec2(&accessors, &buffer_views, bin, acc))
                .unwrap_or_default();
            let indices = read_accessor_indices(&accessors, &buffer_views, bin, idx_acc);

            // Convert indexed triangles to quads
            let mut i = 0;
            while i + 2 < indices.len() {
                let i0 = indices[i] as usize;
                let i1 = indices[i + 1] as usize;
                let i2 = indices[i + 2] as usize;

                let get_pos = |idx: usize| positions.get(idx).copied().unwrap_or(Vec3::ZERO);
                let get_uv = |idx: usize| texcoords.get(idx).copied().unwrap_or(Vec2::ZERO);

                // Try to pair adjacent triangles into quads
                if i + 5 < indices.len() {
                    let i3 = indices[i + 3] as usize;
                    let i4 = indices[i + 4] as usize;
                    let i5 = indices[i + 5] as usize;

                    // Check if two triangles share an edge and are coplanar
                    let n1 = (get_pos(i1) - get_pos(i0)).cross(get_pos(i2) - get_pos(i0));
                    let n2 = (get_pos(i4) - get_pos(i3)).cross(get_pos(i5) - get_pos(i3));
                    let coplanar = n1.normalize_or_zero().dot(n2.normalize_or_zero()) > 0.99;

                    // Check shared edge: i0==i3 && i2==i4 (common strip pattern)
                    let shared = (i0 == i3 && i2 == i4) || (i0 == i5 && i2 == i3) || (i1 == i3 && i2 == i5);

                    if coplanar && shared {
                        // Find the unique fourth vertex
                        let quad_verts = if i0 == i3 && i2 == i4 {
                            [i0, i1, i2, i5]
                        } else if i0 == i5 && i2 == i3 {
                            [i0, i1, i2, i4]
                        } else {
                            [i0, i1, i5, i2]
                        };
                        faces.push(Face {
                            positions: [get_pos(quad_verts[0]), get_pos(quad_verts[1]), get_pos(quad_verts[2]), get_pos(quad_verts[3])],
                            uvs: [get_uv(quad_verts[0]), get_uv(quad_verts[1]), get_uv(quad_verts[2]), get_uv(quad_verts[3])],
                            colors: [Vec4::ONE; 4],
                            hidden: false,
                        });
                        i += 6;
                        continue;
                    }
                }

                // Single triangle → degenerate quad
                faces.push(Face {
                    positions: [get_pos(i0), get_pos(i1), get_pos(i2), get_pos(i2)],
                    uvs: [get_uv(i0), get_uv(i1), get_uv(i2), get_uv(i2)],
                    colors: [Vec4::ONE; 4],
                    hidden: false,
                });
                i += 3;
            }
        }

        if !faces.is_empty() {
            objects.push((faces, mesh.name.clone()));
        }
    }

    if objects.is_empty() {
        return Err("No geometry found in GLB file".to_string());
    }

    Ok(objects)
}

// --- GLB parsing helpers ---

struct GlbBufferView {
    byte_offset: usize,
    _byte_length: usize,
}

struct GlbAccessor {
    buffer_view: usize,
    component_type: u32,
    count: usize,
    _accessor_type: String,
}

struct GlbMesh {
    name: Option<String>,
    position_accessor: Option<usize>,
    texcoord_accessor: Option<usize>,
    indices_accessor: Option<usize>,
}

fn parse_json_number(s: &str) -> usize {
    s.trim().trim_matches(|c: char| !c.is_ascii_digit()).parse().unwrap_or(0)
}

fn parse_glb_buffer_views(json: &str) -> Vec<GlbBufferView> {
    let mut views = Vec::new();
    let Some(start) = json.find("\"bufferViews\"") else { return views };
    let Some(arr_start) = json[start..].find('[') else { return views };
    let json_slice = &json[start + arr_start..];
    let Some(arr_end) = find_matching_bracket(json_slice) else { return views };
    let arr = &json_slice[1..arr_end];

    for obj in split_json_objects(arr) {
        let byte_offset = extract_json_field(&obj, "byteOffset").map(|s| parse_json_number(&s)).unwrap_or(0);
        let byte_length = extract_json_field(&obj, "byteLength").map(|s| parse_json_number(&s)).unwrap_or(0);
        views.push(GlbBufferView { byte_offset, _byte_length: byte_length });
    }
    views
}

fn parse_glb_accessors(json: &str) -> Vec<GlbAccessor> {
    let mut accessors = Vec::new();
    let Some(start) = json.find("\"accessors\"") else { return accessors };
    let Some(arr_start) = json[start..].find('[') else { return accessors };
    let json_slice = &json[start + arr_start..];
    let Some(arr_end) = find_matching_bracket(json_slice) else { return accessors };
    let arr = &json_slice[1..arr_end];

    for obj in split_json_objects(arr) {
        let buffer_view = extract_json_field(&obj, "bufferView").map(|s| parse_json_number(&s)).unwrap_or(0);
        let component_type = extract_json_field(&obj, "componentType").map(|s| parse_json_number(&s) as u32).unwrap_or(0);
        let count = extract_json_field(&obj, "count").map(|s| parse_json_number(&s)).unwrap_or(0);
        let accessor_type = extract_json_string(&obj, "type").unwrap_or_default();
        accessors.push(GlbAccessor { buffer_view, component_type, count, _accessor_type: accessor_type });
    }
    accessors
}

fn parse_glb_meshes(json: &str) -> Vec<GlbMesh> {
    let mut meshes = Vec::new();
    let Some(start) = json.find("\"meshes\"") else { return meshes };
    let Some(arr_start) = json[start..].find('[') else { return meshes };
    let json_slice = &json[start + arr_start..];
    let Some(arr_end) = find_matching_bracket(json_slice) else { return meshes };
    let arr = &json_slice[1..arr_end];

    for obj in split_json_objects(arr) {
        let name = extract_json_string(&obj, "name");
        let position_accessor = extract_json_field(&obj, "POSITION").map(|s| parse_json_number(&s));
        let texcoord_accessor = extract_json_field(&obj, "TEXCOORD_0").map(|s| parse_json_number(&s));
        let indices_accessor = extract_json_field(&obj, "indices").map(|s| parse_json_number(&s));
        meshes.push(GlbMesh { name, position_accessor, texcoord_accessor, indices_accessor });
    }
    meshes
}

fn find_matching_bracket(s: &str) -> Option<usize> {
    let open = s.as_bytes()[0];
    let close = if open == b'[' { b']' } else { b'}' };
    let mut depth = 0;
    for (i, ch) in s.bytes().enumerate() {
        if ch == open { depth += 1; }
        if ch == close { depth -= 1; if depth == 0 { return Some(i); } }
    }
    None
}

fn split_json_objects(s: &str) -> Vec<String> {
    let mut objects = Vec::new();
    let mut depth = 0;
    let mut start = None;
    for (i, ch) in s.char_indices() {
        if ch == '{' {
            if depth == 0 { start = Some(i); }
            depth += 1;
        }
        if ch == '}' {
            depth -= 1;
            if depth == 0
                && let Some(s_idx) = start {
                    objects.push(s[s_idx..=i].to_string());
                }
        }
    }
    objects
}

fn extract_json_field(obj: &str, key: &str) -> Option<String> {
    let search = format!("\"{}\"", key);
    let idx = obj.find(&search)?;
    let after_key = &obj[idx + search.len()..];
    let colon = after_key.find(':')?;
    let value_start = &after_key[colon + 1..].trim_start();
    // Read until comma, closing brace, or end
    let end = value_start.find([',', '}', ']']).unwrap_or(value_start.len());
    Some(value_start[..end].trim().to_string())
}

fn extract_json_string(obj: &str, key: &str) -> Option<String> {
    let field = extract_json_field(obj, key)?;
    let trimmed = field.trim().trim_matches('"');
    if trimmed.is_empty() { None } else { Some(trimmed.to_string()) }
}

fn read_accessor_vec3(accessors: &[GlbAccessor], views: &[GlbBufferView], bin: &[u8], acc_idx: usize) -> Vec<Vec3> {
    let acc = match accessors.get(acc_idx) { Some(a) => a, None => return Vec::new() };
    let view = match views.get(acc.buffer_view) { Some(v) => v, None => return Vec::new() };
    let start = view.byte_offset;
    let mut result = Vec::with_capacity(acc.count);
    for i in 0..acc.count {
        let off = start + i * 12;
        if off + 12 > bin.len() { break; }
        let x = f32::from_le_bytes([bin[off], bin[off+1], bin[off+2], bin[off+3]]);
        let y = f32::from_le_bytes([bin[off+4], bin[off+5], bin[off+6], bin[off+7]]);
        let z = f32::from_le_bytes([bin[off+8], bin[off+9], bin[off+10], bin[off+11]]);
        result.push(Vec3::new(x, y, z));
    }
    result
}

fn read_accessor_vec2(accessors: &[GlbAccessor], views: &[GlbBufferView], bin: &[u8], acc_idx: usize) -> Vec<Vec2> {
    let acc = match accessors.get(acc_idx) { Some(a) => a, None => return Vec::new() };
    let view = match views.get(acc.buffer_view) { Some(v) => v, None => return Vec::new() };
    let start = view.byte_offset;
    let mut result = Vec::with_capacity(acc.count);
    for i in 0..acc.count {
        let off = start + i * 8;
        if off + 8 > bin.len() { break; }
        let u = f32::from_le_bytes([bin[off], bin[off+1], bin[off+2], bin[off+3]]);
        let v = f32::from_le_bytes([bin[off+4], bin[off+5], bin[off+6], bin[off+7]]);
        result.push(Vec2::new(u, v));
    }
    result
}

fn read_accessor_indices(accessors: &[GlbAccessor], views: &[GlbBufferView], bin: &[u8], acc_idx: usize) -> Vec<u32> {
    let acc = match accessors.get(acc_idx) { Some(a) => a, None => return Vec::new() };
    let view = match views.get(acc.buffer_view) { Some(v) => v, None => return Vec::new() };
    let start = view.byte_offset;
    let mut result = Vec::with_capacity(acc.count);
    match acc.component_type {
        5125 => { // UNSIGNED_INT
            for i in 0..acc.count {
                let off = start + i * 4;
                if off + 4 > bin.len() { break; }
                result.push(u32::from_le_bytes([bin[off], bin[off+1], bin[off+2], bin[off+3]]));
            }
        }
        5123 => { // UNSIGNED_SHORT
            for i in 0..acc.count {
                let off = start + i * 2;
                if off + 2 > bin.len() { break; }
                result.push(u16::from_le_bytes([bin[off], bin[off+1]]) as u32);
            }
        }
        5121 => { // UNSIGNED_BYTE
            for i in 0..acc.count {
                let off = start + i;
                if off >= bin.len() { break; }
                result.push(bin[off] as u32);
            }
        }
        _ => {}
    }
    result
}

/// Export the scene as a GLB (binary glTF 2.0) file.
pub fn export_glb(scene: &Scene, path: &Path) -> Result<(), String> {
    // Collect per-object geometry into a single binary buffer
    let mut bin: Vec<u8> = Vec::new();

    // JSON building blocks
    let mut json_accessors = Vec::new();
    let mut json_buffer_views = Vec::new();
    let mut json_meshes = Vec::new();
    let mut json_nodes = Vec::new();
    let mut node_indices = Vec::new();

    for layer in &scene.layers {
        if !layer.visible { continue; }
        for object in &layer.objects {
            let visible_faces: Vec<_> = object.faces.iter().filter(|f| !f.hidden).collect();
            if visible_faces.is_empty() { continue; }

            let vertex_count = visible_faces.len() * 4;
            let index_count = visible_faces.len() * 6;

            let mut positions: Vec<f32> = Vec::with_capacity(vertex_count * 3);
            let mut texcoords: Vec<f32> = Vec::with_capacity(vertex_count * 2);
            let mut colors: Vec<f32> = Vec::with_capacity(vertex_count * 4);
            let mut indices: Vec<u32> = Vec::with_capacity(index_count);

            let mut min_pos = [f32::MAX; 3];
            let mut max_pos = [f32::MIN; 3];

            for face in &visible_faces {
                let base = (positions.len() / 3) as u32;
                for i in 0..4 {
                    let p = face.positions[i];
                    positions.extend_from_slice(&[p.x, p.y, p.z]);
                    min_pos[0] = min_pos[0].min(p.x);
                    min_pos[1] = min_pos[1].min(p.y);
                    min_pos[2] = min_pos[2].min(p.z);
                    max_pos[0] = max_pos[0].max(p.x);
                    max_pos[1] = max_pos[1].max(p.y);
                    max_pos[2] = max_pos[2].max(p.z);
                    texcoords.extend_from_slice(&[face.uvs[i].x, face.uvs[i].y]);
                    let c = face.colors[i];
                    colors.extend_from_slice(&[c.x, c.y, c.z, c.w]);
                }
                indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
            }

            // Helper: append bytes to bin and return (offset, byte_length), aligned to 4
            let mut append = |data: &[u8]| -> (usize, usize) {
                let offset = bin.len();
                bin.extend_from_slice(data);
                let len = data.len();
                while !bin.len().is_multiple_of(4) { bin.push(0); }
                (offset, len)
            };

            // Position buffer view + accessor
            let (pos_off, pos_len) = append(bytemuck::cast_slice::<f32, u8>(&positions));
            let pos_bv = json_buffer_views.len();
            json_buffer_views.push(format!(
                r#"{{"buffer":0,"byteOffset":{},"byteLength":{},"target":34962}}"#,
                pos_off, pos_len
            ));
            let pos_acc = json_accessors.len();
            json_accessors.push(format!(
                r#"{{"bufferView":{},"componentType":5126,"count":{},"type":"VEC3","min":[{},{},{}],"max":[{},{},{}]}}"#,
                pos_bv, vertex_count,
                min_pos[0], min_pos[1], min_pos[2],
                max_pos[0], max_pos[1], max_pos[2],
            ));

            // Texcoord buffer view + accessor
            let (tc_off, tc_len) = append(bytemuck::cast_slice::<f32, u8>(&texcoords));
            let tc_bv = json_buffer_views.len();
            json_buffer_views.push(format!(
                r#"{{"buffer":0,"byteOffset":{},"byteLength":{},"target":34962}}"#,
                tc_off, tc_len
            ));
            let tc_acc = json_accessors.len();
            json_accessors.push(format!(
                r#"{{"bufferView":{},"componentType":5126,"count":{},"type":"VEC2"}}"#,
                tc_bv, vertex_count,
            ));

            // Color buffer view + accessor
            let (col_off, col_len) = append(bytemuck::cast_slice::<f32, u8>(&colors));
            let col_bv = json_buffer_views.len();
            json_buffer_views.push(format!(
                r#"{{"buffer":0,"byteOffset":{},"byteLength":{},"target":34962}}"#,
                col_off, col_len
            ));
            let col_acc = json_accessors.len();
            json_accessors.push(format!(
                r#"{{"bufferView":{},"componentType":5126,"count":{},"type":"VEC4"}}"#,
                col_bv, vertex_count,
            ));

            // Index buffer view + accessor
            let (idx_off, idx_len) = append(bytemuck::cast_slice::<u32, u8>(&indices));
            let idx_bv = json_buffer_views.len();
            json_buffer_views.push(format!(
                r#"{{"buffer":0,"byteOffset":{},"byteLength":{},"target":34963}}"#,
                idx_off, idx_len
            ));
            let idx_acc = json_accessors.len();
            json_accessors.push(format!(
                r#"{{"bufferView":{},"componentType":5125,"count":{},"type":"SCALAR"}}"#,
                idx_bv, index_count,
            ));

            // Mesh
            let mesh_idx = json_meshes.len();
            let escaped_name = object.name.replace('\\', "\\\\").replace('"', "\\\"");
            json_meshes.push(format!(
                r#"{{"name":"{}","primitives":[{{"attributes":{{"POSITION":{},"TEXCOORD_0":{},"COLOR_0":{}}},"indices":{},"mode":4}}]}}"#,
                escaped_name, pos_acc, tc_acc, col_acc, idx_acc,
            ));

            // Node
            let node_idx = json_nodes.len();
            json_nodes.push(format!(
                r#"{{"name":"{}","mesh":{}}}"#,
                escaped_name, mesh_idx,
            ));
            node_indices.push(node_idx);
        }
    }

    if json_meshes.is_empty() {
        return Err("No visible geometry to export".to_string());
    }

    // Build JSON string
    let node_list: Vec<String> = node_indices.iter().map(|i| i.to_string()).collect();
    let mut json = String::new();
    write!(json, r#"{{"asset":{{"version":"2.0","generator":"Cracktile 3D"}}"#).unwrap();
    write!(json, r#","scene":0,"scenes":[{{"nodes":[{}]}}]"#, node_list.join(",")).unwrap();
    write!(json, r#","nodes":[{}]"#, json_nodes.join(",")).unwrap();
    write!(json, r#","meshes":[{}]"#, json_meshes.join(",")).unwrap();
    write!(json, r#","accessors":[{}]"#, json_accessors.join(",")).unwrap();
    write!(json, r#","bufferViews":[{}]"#, json_buffer_views.join(",")).unwrap();
    write!(json, r#","buffers":[{{"byteLength":{}}}]}}"#, bin.len()).unwrap();

    // Pad JSON to 4-byte alignment
    let mut json_bytes = json.into_bytes();
    while !json_bytes.len().is_multiple_of(4) { json_bytes.push(b' '); }

    // Pad BIN to 4-byte alignment
    while !bin.len().is_multiple_of(4) { bin.push(0); }

    // GLB structure: header(12) + json_chunk_header(8) + json + bin_chunk_header(8) + bin
    let total_length = 12 + 8 + json_bytes.len() + 8 + bin.len();

    let mut file = fs::File::create(path)
        .map_err(|e| format!("Create failed: {e}"))?;

    // GLB header
    file.write_all(b"glTF").map_err(|e| format!("Write failed: {e}"))?;          // magic
    file.write_all(&2u32.to_le_bytes()).map_err(|e| format!("Write failed: {e}"))?; // version
    file.write_all(&(total_length as u32).to_le_bytes()).map_err(|e| format!("Write failed: {e}"))?; // total length

    // JSON chunk
    file.write_all(&(json_bytes.len() as u32).to_le_bytes()).map_err(|e| format!("Write failed: {e}"))?;
    file.write_all(&0x4E4F534Au32.to_le_bytes()).map_err(|e| format!("Write failed: {e}"))?; // "JSON"
    file.write_all(&json_bytes).map_err(|e| format!("Write failed: {e}"))?;

    // BIN chunk
    file.write_all(&(bin.len() as u32).to_le_bytes()).map_err(|e| format!("Write failed: {e}"))?;
    file.write_all(&0x004E4942u32.to_le_bytes()).map_err(|e| format!("Write failed: {e}"))?; // "BIN\0"
    file.write_all(&bin).map_err(|e| format!("Write failed: {e}"))?;

    Ok(())
}
