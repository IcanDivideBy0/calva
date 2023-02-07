struct Camera {
    view: mat4x4<f32>,
    proj: mat4x4<f32>,
    view_proj: mat4x4<f32>,
    inv_view: mat4x4<f32>,
    inv_proj: mat4x4<f32>,
}

@group(0) @binding(0) var<uniform> camera: Camera;
@group(1) @binding(0) var<uniform> model_matrix: mat4x4<f32>;

@vertex
fn vs_main(@location(0) pos: vec3<f32>) -> @builtin(position) vec4<f32> {
  return camera.view_proj * model_matrix * vec4<f32>(pos, 1.0);
}

var<push_constant> color: vec4<f32>;
@fragment
fn fs_main() -> @location(0) vec4<f32> {
  return color;
}