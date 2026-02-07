use std::fmt::Write as FmtWrite;
use std::fs;
use std::io::Write as IoWrite;
use std::path::Path;
use crate::scene::Scene;

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
