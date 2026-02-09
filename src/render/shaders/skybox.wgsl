struct SkyboxUniform {
    inv_vp: mat4x4<f32>,
    top_color: vec4<f32>,
    bottom_color: vec4<f32>,
    // x = mode (0.0 = gradient, 1.0 = equirect texture)
    params: vec4<f32>,
};

@group(0) @binding(0) var<uniform> skybox: SkyboxUniform;
@group(1) @binding(0) var t_skybox: texture_2d<f32>;
@group(1) @binding(1) var s_skybox: sampler;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) clip_pos: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    // Fullscreen triangle: 3 vertices covering the entire screen
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    let p = positions[vertex_index];

    var out: VertexOutput;
    out.position = vec4<f32>(p.x, p.y, 0.999, 1.0);
    out.clip_pos = p;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Reconstruct world-space ray direction from clip-space position
    let near_clip = vec4<f32>(in.clip_pos, 0.0, 1.0);
    let far_clip = vec4<f32>(in.clip_pos, 1.0, 1.0);
    let near_world = skybox.inv_vp * near_clip;
    let far_world = skybox.inv_vp * far_clip;
    let dir = normalize(far_world.xyz / far_world.w - near_world.xyz / near_world.w);

    if skybox.params.x < 0.5 {
        // Gradient mode: blend between bottom and top based on Y direction
        let t = dir.y * 0.5 + 0.5; // Map [-1, 1] to [0, 1]
        return mix(skybox.bottom_color, skybox.top_color, t);
    } else {
        // Equirectangular texture mode
        let phi = atan2(dir.z, dir.x);
        let theta = asin(clamp(dir.y, -1.0, 1.0));
        let u = phi / (2.0 * 3.14159265) + 0.5;
        let v = 0.5 - theta / 3.14159265;
        return textureSample(t_skybox, s_skybox, vec2<f32>(u, v));
    }
}
