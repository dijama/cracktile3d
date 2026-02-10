struct CameraUniform {
    view_proj: mat4x4<f32>,
};

struct LightUniform {
    direction: vec4<f32>,   // xyz = direction (toward light), w = enabled (1.0 or 0.0)
    color: vec4<f32>,       // rgb = light color, a = intensity
    ambient: vec4<f32>,     // rgb = ambient color, a = unused
};

struct ModelUniform {
    model: mat4x4<f32>,
};

@group(0) @binding(0) var<uniform> camera: CameraUniform;
@group(1) @binding(0) var t_tileset: texture_2d<f32>;
@group(1) @binding(1) var s_tileset: sampler;
@group(2) @binding(0) var<uniform> light: LightUniform;
@group(3) @binding(0) var<uniform> model: ModelUniform;

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

// Compute the cofactor (adjugate) of a 3x3 matrix — transpose of this is the
// inverse-transpose needed for correct normal transformation under non-uniform scale.
fn cofactor3(m: mat3x3<f32>) -> mat3x3<f32> {
    let c0 = cross(m[1], m[2]);
    let c1 = cross(m[2], m[0]);
    let c2 = cross(m[0], m[1]);
    return mat3x3<f32>(c0, c1, c2);
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let world_pos = model.model * vec4<f32>(in.position, 1.0);
    out.clip_position = camera.view_proj * world_pos;
    out.uv = in.uv;
    out.color = in.color;
    // Normal matrix = cofactor of upper-left 3x3 (equivalent to transpose(inverse(M)) * det(M))
    // The determinant factor cancels after normalize(), so cofactor alone is sufficient.
    let model3 = mat3x3<f32>(model.model[0].xyz, model.model[1].xyz, model.model[2].xyz);
    out.normal = normalize(cofactor3(model3) * in.normal);
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
