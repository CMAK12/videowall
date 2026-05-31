// Fullscreen-triangle vertex shader paired with a YUV→RGB fragment shader.
// The chroma textures are half-size; the sampler does bilinear chroma
// upsampling for free.

struct VsOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VsOut {
    // Single triangle covering the clip-space square [-1,1]^2.
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 3.0, -1.0),
        vec2<f32>(-1.0,  3.0),
    );
    // UVs chosen so the visible region [-1,1] maps to [0,1] in y-flipped space
    // (wgpu texture origin is top-left, NDC y points up).
    var uvs = array<vec2<f32>, 3>(
        vec2<f32>(0.0, 1.0),
        vec2<f32>(2.0, 1.0),
        vec2<f32>(0.0, -1.0),
    );
    var out: VsOut;
    out.clip_pos = vec4<f32>(positions[vi], 0.0, 1.0);
    out.uv = uvs[vi];
    return out;
}

@group(0) @binding(0) var samp: sampler;
@group(0) @binding(1) var y_tex: texture_2d<f32>;
@group(0) @binding(2) var u_tex: texture_2d<f32>;
@group(0) @binding(3) var v_tex: texture_2d<f32>;

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let y = textureSample(y_tex, samp, in.uv).r;
    let u = textureSample(u_tex, samp, in.uv).r;
    let v = textureSample(v_tex, samp, in.uv).r;

    // BT.601 limited-range YUV → linear RGB.
    let y_lin = (y - 16.0 / 255.0) * (255.0 / 219.0);
    let u_off = (u - 128.0 / 255.0) * (255.0 / 224.0);
    let v_off = (v - 128.0 / 255.0) * (255.0 / 224.0);

    let r = y_lin + 1.402 * v_off;
    let g = y_lin - 0.344136 * u_off - 0.714136 * v_off;
    let b = y_lin + 1.772 * u_off;

    return vec4<f32>(r, g, b, 1.0);
}
