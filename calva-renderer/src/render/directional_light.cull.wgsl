struct Camera {
    view: mat4x4<f32>,
    proj: mat4x4<f32>,
    view_proj: mat4x4<f32>,
    inv_view: mat4x4<f32>,
    inv_proj: mat4x4<f32>,
    frustum: array<vec4<f32>, 6>,
}
@group(0) @binding(0) var<uniform> camera: Camera;

struct DirectionalLight {
    color: vec4<f32>,
    direction_world: vec4<f32>,
    direction_view: vec4<f32>,
    view_proj: mat4x4<f32>,
}
@group(1) @binding(0) var<uniform> directional_light: DirectionalLight;

struct MeshBoundingSphere {
    center: vec3<f32>,
    radius: f32,
}

struct MeshInfo {
    vertex_count: u32,
    base_index: u32,
    vertex_offset: i32,
    skin_offset: i32,
    bounding_sphere: MeshBoundingSphere,
}

struct AnimationState {
    animation_id: u32,
    time: f32,
}

struct Instance {
    transform: mat4x4<f32>,
    mesh_id: u32,
    material_id: u32,
    animation: AnimationState,
}
struct Instances {
    count: u32,
    instances: array<Instance>
}

struct CullInfo {
    view_proj: mat4x4<f32>,
    frustum: array<vec4<f32>, 6>,
}

struct DrawInstance {
    transform: mat4x4<f32>,
    material_id: u32,
    skin_offset: i32,
    animation: AnimationState,
}

struct DrawIndexedIndirect {
    vertex_count: u32,
    instance_count: atomic<u32>,
    base_index: u32,
    vertex_offset: i32,
    base_instance: u32,
}
struct DrawIndirects {
    count: atomic<u32>,
    draws: array<DrawIndexedIndirect>,
}

@group(2) @binding(0)
var<storage, read> meshes_info: array<MeshInfo>;

@group(2) @binding(1)
var<storage, read> base_instances: array<u32>;

@group(2) @binding(2)
var<storage, read> instances: Instances;

@group(2) @binding(3)
var<storage, read_write> draw_instances: array<DrawInstance>;

@group(2) @binding(4)
var<storage, read_write> draw_indirects: DrawIndirects;

@compute @workgroup_size(32)
fn reset(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let mesh_id = global_id.x;
    let mesh_info = &meshes_info[mesh_id];
    let draw = &draw_indirects.draws[mesh_id];

    (*draw).vertex_count = (*mesh_info).vertex_count;
    (*draw).instance_count = 0u;
    (*draw).base_index = (*mesh_info).base_index;
    (*draw).vertex_offset = (*mesh_info).vertex_offset;
    (*draw).base_instance = base_instances[mesh_id];

    draw_indirects.count = 0u;
}

fn plane_distance_to_point(plane: vec4<f32>, p: vec3<f32>) -> f32 {
    return dot(plane.xyz, p) + plane.w;
}
fn sphere_visible(sphere: MeshBoundingSphere, transform: mat4x4<f32>) -> bool {
    let center_world = transform * vec4<f32>(sphere.center, 1.0);

    let c = directional_light.view_proj * center_world;
    let center_light_view = c.xyz;

    // This is orthographic projection, so we can project radius into clip space to measure its length
    let r = directional_light.view_proj * transform * vec4<f32>(sphere.radius, 0.0, 0.0, 1.0);
    let radius_light_view = length(r.xyz - center_light_view);

    if length(center_light_view.xy) - radius_light_view > 1.0 {
        return false;
    }

    let det = determinant(transform);
    let scale = vec3<f32>(
        length(transform[0].xyz) * sign(det),
        length(transform[1].xyz),
        length(transform[2].xyz),
    );
    let abs_scale = abs(scale);
    let max_scale = max(max(scale.x, scale.y), scale.z);
    let neg_radius = -(sphere.radius * max_scale);

    let light_dir = directional_light.direction_world.xyz;
    for (var i = 0u; i < 6u; i++) {
        let plane_normal = camera.frustum[i].xyz;

        if dot(plane_normal, light_dir) < 0.0 {
            if plane_distance_to_point(camera.frustum[i], center_world.xyz) < neg_radius {
                return false;
            }
        }
    }

    return true;
}

@compute @workgroup_size(32)
fn cull(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let instance_index = global_id.x;

    if instance_index >= instances.count {
        return;
    }

    let instance = &instances.instances[instance_index];
    let transform = &(*instance).transform;
    let mesh_id = (*instance).mesh_id;
    let mesh_info = &meshes_info[mesh_id];

    if !sphere_visible((*mesh_info).bounding_sphere, (*transform)) {
        return;
    }

    let draw = &draw_indirects.draws[mesh_id];
    let draw_instance_index = (*draw).base_instance + atomicAdd(&(*draw).instance_count, 1u);

    let draw_instance = &draw_instances[draw_instance_index];
    (*draw_instance).transform = *transform;
    (*draw_instance).material_id = (*instance).material_id;
    (*draw_instance).skin_offset = (*mesh_info).skin_offset;
    (*draw_instance).animation = (*instance).animation;
}

@compute @workgroup_size(32)
fn count(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let mesh_id = global_id.x;

    let draw = &draw_indirects.draws[mesh_id];
    let copy = *draw;

    if (*draw).instance_count > 0u {
        draw_indirects.draws[atomicAdd(&draw_indirects.count, 1u)] = copy;
    }
}
