import package::util::math::{plane_distance_to_point, axis_quat};
import package::resources::{
    instances::{
        Instances,
        instance_active,
        instance_mesh_id,
        instance_material_id,
    },
    camera::Camera,
    mesh::{MeshInfo, MeshBoundingSphere},
    animation::AnimationState,
};

@group(0) @binding(0) var<uniform> camera: Camera;

struct DrawInstance {
    transform: mat4x4<f32>,
    normal_quat: vec4<f32>,
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

@group(1) @binding(0)
var<storage, read> meshes_info: array<MeshInfo>;

@group(1) @binding(1)
var<storage, read> base_instances: array<u32>;

@group(1) @binding(2)
var<storage, read> instances: Instances;

@group(1) @binding(3)
var<storage, read_write> draw_instances: array<DrawInstance>;

@group(1) @binding(4)
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

fn sphere_visible(sphere: MeshBoundingSphere, transform: mat4x4<f32>, scale: vec3<f32>) -> bool {
    let p = transform * vec4<f32>(sphere.center, 1.0);
    let pos = p.xyz / p.w;

    let abs_scale = abs(scale);
    let max_scale = max(max(scale.x, scale.y), scale.z);
    let neg_radius = -(sphere.radius * max_scale);

    return !(
        plane_distance_to_point(camera.frustum[0], pos) < neg_radius ||
        plane_distance_to_point(camera.frustum[1], pos) < neg_radius ||
        plane_distance_to_point(camera.frustum[2], pos) < neg_radius ||
        plane_distance_to_point(camera.frustum[3], pos) < neg_radius ||
        plane_distance_to_point(camera.frustum[4], pos) < neg_radius ||
        plane_distance_to_point(camera.frustum[5], pos) < neg_radius
    );
}

@compute @workgroup_size(32)
fn cull(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let instance_index = global_id.x;

    if instance_index >= instances.count {
        return;
    }

    let instance = &instances.instances[instance_index];

    if !instance_active(*instance) {
        return;
    }

    let transform = &(*instance).transform;
    let mesh_id = instance_mesh_id(*instance);
    let mesh_info = &meshes_info[mesh_id];

    // /!\ negative scaling not supported
    let scale = vec3<f32>(
        length(transpose(*transform)[0].xyz),
        length(transpose(*transform)[1].xyz),
        length(transpose(*transform)[2].xyz),
    );

    if !sphere_visible((*mesh_info).bounding_sphere, (*transform), scale) {
        return;
    }

    let draw = &draw_indirects.draws[mesh_id];
    let draw_instance_index = (*draw).base_instance + atomicAdd(&(*draw).instance_count, 1u);

    let draw_instance = &draw_instances[draw_instance_index];
    (*draw_instance).transform = *transform;

    let inv_scale = 1.0 / scale;
    (*draw_instance).normal_quat = axis_quat(
        (*transform)[0].xyz * inv_scale.x,
        (*transform)[1].xyz * inv_scale.y,
        (*transform)[2].xyz * inv_scale.z,
    );

    (*draw_instance).material_id = instance_material_id(*instance);
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
