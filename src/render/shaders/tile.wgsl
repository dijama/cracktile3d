struct CameraUniform {
    view_proj: mat4x4<f32>,
};

struct LightUniform {
    direction: vec4<f32>,   // xyz = direction (toward light), w = enabled (1.0 or 0.0)
    color: vec4<f32>,       // rgb = light color, a = intensity
    ambient: vec4<f32>,     // rgb = ambient color, a = unused
};

@group(0) @binding(0) var<uniform> camera: CameraUniform;
@group(1) @binding(0) var t_tileset: texture_2d<f32>;
@group(1) @binding(1) var s_tileset: sampler;
@group(2) @binding(0) var<uniform> light: LightUniform;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) normal: vec3<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = camera.view_proj * vec4<f32>(in.position, 1.0);
    out.uv = in.uv;
    out.color = in.color;
    out.normal = in.normal;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let tex_color = textureSample(t_tileset, s_tileset, in.uv);
    if tex_color.a < 0.01 {
        discard;
    }

    var base_color = tex_color * in.color;

    // Apply lighting if enabled
    if light.direction.w > 0.5 {
        let n = normalize(in.normal);
        let l = normalize(light.direction.xyz);
        let ndotl = max(dot(n, l), 0.0);
        // Also light the back face (two-sided lighting for tiles)
        let ndotl_back = max(dot(-n, l), 0.0);
        let diffuse = max(ndotl, ndotl_back);
        let lit = light.ambient.rgb + light.color.rgb * light.color.a * diffuse;
        base_color = vec4<f32>(base_color.rgb * lit, base_color.a);
    }

    return base_color;
}
