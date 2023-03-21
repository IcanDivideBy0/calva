//
// Vertex shader
//

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) ndc: vec2<f32>,
    @location(1) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    let tc = vec2<f32>(
        f32(vertex_index >> 1u),
        f32(vertex_index & 1u),
    ) * 2.0;

    var out: VertexOutput;
    out.position = vec4<f32>(tc * 2.0 - 1.0, 0.0, 1.0);
    out.ndc = out.position.xy;
    out.uv = out.ndc * vec2<f32>(0.5, -0.5) + 0.5;

    return out;
}

//
// Fragment shader
//

@group(0) @binding(0) var t_sampler: sampler;
@group(0) @binding(1) var t_input: texture_depth_2d;

fn blur(position: vec4<f32>, direction: vec2<i32>) -> f32 {
    let c = vec2<i32>(position.xy);

    var result: f32 = 0.0;

    result = result + textureLoad(t_input, c + vec2<i32>(-3) * direction, 0) * ( 1.0 / 64.0);
    result = result + textureLoad(t_input, c + vec2<i32>(-2) * direction, 0) * ( 6.0 / 64.0);
    result = result + textureLoad(t_input, c + vec2<i32>(-1) * direction, 0) * (15.0 / 64.0);
    result = result + textureLoad(t_input, c + vec2<i32>( 0) * direction, 0) * (20.0 / 64.0);
    result = result + textureLoad(t_input, c + vec2<i32>( 1) * direction, 0) * (15.0 / 64.0);
    result = result + textureLoad(t_input, c + vec2<i32>( 2) * direction, 0) * ( 6.0 / 64.0);
    result = result + textureLoad(t_input, c + vec2<i32>( 3) * direction, 0) * ( 1.0 / 64.0);

    return result;
}

@fragment
fn fs_main_horizontal(in: VertexOutput) -> @builtin(frag_depth) f32 {
    return blur(in.position, vec2<i32>(1, 0));
}

@fragment
fn fs_main_vertical(in: VertexOutput) -> @builtin(frag_depth) f32 {
    return blur(in.position, vec2<i32>(0, 1));
}

// @fragment
// fn fs_main_horizontal(in: VertexOutput) -> @builtin(frag_depth) f32 {
//     return textureLoad(t_input, vec2<i32>(in.position.xy), 0);
// }

// const TAU: f32 = 6.28318530718; // 2π

// const DIRECTIONS: f32 = 6.0;   // More is better but slower
// const QUALITY: f32 = 3.0;       // More is better but slower
// const SIZE: f32 = 4.0;          // Radius

// @fragment
// fn fs_main_vertical(in: VertexOutput) -> @builtin(frag_depth) f32 {
//     let t_dim = vec2<f32>(textureDimensions(t_input));
//     let radius = SIZE / t_dim;

//     var color = textureSample(t_input, t_sampler, in.uv);
//     var acc = 1.0;
//     for (var d = 0.0; d < TAU; d += TAU / DIRECTIONS) {
//         for(var i = 1.0 / QUALITY; i <= 1.0; i += 1.0 / QUALITY) {
//             color += textureSample(t_input, t_sampler, in.uv + vec2<f32>(cos(d), sin(d)) * radius * i);
//             acc += 1.0;
//         }
//     }

//     color /= acc;
//     return color;
// }
