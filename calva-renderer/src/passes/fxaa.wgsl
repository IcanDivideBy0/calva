//
// Vertex shader
//

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> @builtin(position) vec4<f32> {
    let tc = vec2<f32>(
        f32(vertex_index >> 1u),
        f32(vertex_index & 1u),
    ) * 2.0;

    return vec4<f32>(tc * 2.0 - 1.0, 0.0, 1.0);
}

//
// Fragment shader
//

@group(0) @binding(0) var t_sampler: sampler;
@group(0) @binding(1) var t_input: texture_2d<f32>;

const LUMA: vec3<f32> = vec3<f32>(0.299, 0.587, 0.114);
const SPAN_MIN: vec2<f32> = vec2<f32>(-8.0, -8.0);
const SPAN_MAX: vec2<f32> = vec2<f32>( 8.0,  8.0);
const REDUCE_MIN: f32 = 0.0078125; // 1.0 / 128.0
const REDUCE_MUL: f32 = 0.125; // 1.0 / 8.0

@fragment
fn fs_main(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let coord = vec2<i32>(position.xy);
    let luma_tl = dot(LUMA, textureLoad(t_input, coord + vec2<i32>(-1, -1), 0).rgb);
    let luma_tr = dot(LUMA, textureLoad(t_input, coord + vec2<i32>( 1, -1), 0).rgb);
    let luma_bl = dot(LUMA, textureLoad(t_input, coord + vec2<i32>(-1,  1), 0).rgb);
    let luma_br = dot(LUMA, textureLoad(t_input, coord + vec2<i32>( 1,  1), 0).rgb);
    let luma_c  = dot(LUMA, textureLoad(t_input, coord, 0).rgb);

    let luma_min = min(luma_c, min(
        min(luma_tl, luma_tr),
        min(luma_bl, luma_br),
    ));
    let luma_max = max(luma_c, max(
        max(luma_tl, luma_tr),
        max(luma_bl, luma_br),
    ));

    var dir = vec2<f32>(
        -((luma_tl + luma_tr) - (luma_bl + luma_br)),
         ((luma_tl + luma_bl) - (luma_tr + luma_br)),
    );

    let texel_size = 1.0 / vec2<f32>(textureDimensions(t_input));

    let dir_reduce = max((luma_tl + luma_tr + luma_bl + luma_br) * 0.25 * REDUCE_MUL, REDUCE_MIN);
    let temp = min(abs(dir.x), abs(dir.y)) + dir_reduce;
    dir = clamp(dir / temp, SPAN_MIN, SPAN_MAX) * texel_size;

    let uv = position.xy * texel_size;
    let r1 = 0.5 * (
        textureSample(t_input, t_sampler, uv + dir * vec2<f32>(1.0 / 3.0 - 0.5)).rgb +
        textureSample(t_input, t_sampler, uv + dir * vec2<f32>(2.0 / 3.0 - 0.5)).rgb
    );
    let r2 = 0.5 * (
        textureSample(t_input, t_sampler, uv + dir * vec2<f32>(-0.5)).rgb +
        textureSample(t_input, t_sampler, uv + dir * vec2<f32>( 0.5)).rgb
    );
    let r_avg = (r1 + r2) * 0.5;

    let luma_result = dot(LUMA, r_avg);

    let color = select(
        r1,     // false
        r_avg, // true
        luma_min < luma_result && luma_result < luma_max
    );

    return vec4<f32>(color, 1.0);
}