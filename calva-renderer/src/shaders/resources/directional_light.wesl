struct DirectionalLight {
    color: vec4<f32>,
    direction_world: vec4<f32>,
    direction_view: vec4<f32>,
    view_proj: mat4x4<f32>,
}