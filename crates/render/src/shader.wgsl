// CPU uploads four f32 columns, 64 bytes. World +Y up; camera -Z forward.
// Column vectors: projection * view * identity_model * position. No shader transpose.
struct Camera { matrix: mat4x4<f32> };
@group(0) @binding(0) var<uniform> camera: Camera;
@group(0) @binding(1) var atlas: texture_2d<f32>;
@group(0) @binding(2) var sampler_atlas: sampler;
struct In {
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) shade: f32,
    @location(3) color: vec3<f32>,
};
struct Out {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) shade: f32,
    @location(2) color: vec3<f32>,
};
@vertex fn vs_main(input: In) -> Out {
    var out: Out;
    out.position = camera.matrix * vec4<f32>(input.position, 1.0);
    out.uv = input.uv;
    out.shade = input.shade;
    out.color = input.color;
    return out;
}
// Constant per-face vertex colors; no texture read and no lighting.
@fragment fn fs_color(input: Out) -> @location(0) vec4<f32> {
    return vec4<f32>(input.color, 1.0);
}
@fragment fn fs_unlit(input: Out) -> @location(0) vec4<f32> {
    let color = textureSample(atlas, sampler_atlas, input.uv);
    // Material tint is independent of illumination (brightness remains 1).
    return vec4<f32>(color.rgb * input.color, color.a);
}
@fragment fn fs_main(input: Out) -> @location(0) vec4<f32> {
    let color = textureSample(atlas, sampler_atlas, input.uv);
    if color.a < 0.5 { discard; }
    return vec4<f32>(color.rgb * input.color * input.shade, color.a);
}
@fragment fn fs_crack(input: Out) -> @location(0) vec4<f32> {
    let color = textureSample(atlas, sampler_atlas, input.uv);
    if color.a < 0.05 { discard; }
    return vec4<f32>(color.rgb * input.color * input.shade, 0.5);
}
@vertex fn vs_hud(input: In) -> Out {
    var out: Out;
    out.position=vec4<f32>(input.position,1.0);
    out.uv=input.uv; out.shade=input.shade; out.color=input.color;
    return out;
}
@fragment fn fs_hud(input: Out) -> @location(0) vec4<f32> {
    if input.shade < 0.0 { return vec4<f32>(input.color,1.0); }
    let color=textureSample(atlas,sampler_atlas,input.uv);
    if color.a<0.5 {discard;}
    return vec4<f32>(color.rgb*input.color,color.a);
}
// Inspector-only overlay markers use a negative shade; gameplay pipelines are unchanged.
@fragment fn fs_inspect(input: Out) -> @location(0) vec4<f32> {
    if input.shade < 0.0 { return vec4<f32>(input.color,1.0); }
    let color=textureSample(atlas,sampler_atlas,input.uv);
    if color.a<0.5 {discard;}
    return vec4<f32>(color.rgb*input.color*input.shade,color.a);
}
