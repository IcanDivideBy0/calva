var<immediate> tile_size: f32;

@vertex
fn vs_main(@location(0) pos: vec3<f32>) -> @builtin(position) vec4<f32> {{
    return vec4<f32>(
        pos.x / tile_size * 2.0,
        -pos.z / tile_size * 2.0,
        -pos.y / tile_size * 0.5 + 0.5,
        1.0,
    );
}}