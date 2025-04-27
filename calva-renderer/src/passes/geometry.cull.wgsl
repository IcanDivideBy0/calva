struct Camera {
    view: mat4x4<f32>,
    proj: mat4x4<f32>,
    view_proj: mat4x4<f32>,
    inv_view: mat4x4<f32>,
    inv_proj: mat4x4<f32>,
    frustum: array<vec4<f32>, 6>,
}
@group(0) @binding(0) var<uniform> camera: Camera;

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
    /* | deleted  | mat_id   | mesh_id          | */
    /* | 8        | 8        | 16               | */
    packed_data: u32,
    _padding: u32,
    animation: AnimationState,
}
struct Instances {
    count: u32,
    instances: array<Instance>
}

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

fn plane_distance_to_point(plane: vec4<f32>, p: vec3<f32>) -> f32 {
    return dot(plane.xyz, p) + plane.w;
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

fn axis_quat(x_axis: vec3<f32>, y_axis: vec3<f32>, z_axis: vec3<f32>) -> vec4<f32> {
    // Based on https://github.com/microsoft/DirectXMath `XM$quaternionRotationMatrix`
    if z_axis.z <= 0.0 {
        // x^2 + y^2 >= z^2 + w^2
        let dif10 = y_axis.y - x_axis.x;
        let omm22 = 1.0 - z_axis.z;
        if dif10 <= 0.0 {
            // x^2 >= y^2
            let four_xsq = omm22 - dif10;
            let inv4x = 0.5 / sqrt(four_xsq);
            return vec4<f32>(
                four_xsq * inv4x,
                (x_axis.y + y_axis.x) * inv4x,
                (x_axis.z + z_axis.x) * inv4x,
                (y_axis.z - z_axis.y) * inv4x,
            );
        } else {
            // y^2 >= x^2
            let four_ysq = omm22 + dif10;
            let inv4y = 0.5 / sqrt(four_ysq);
            return vec4<f32>(
                (x_axis.y + y_axis.x) * inv4y,
                four_ysq * inv4y,
                (y_axis.z + z_axis.y) * inv4y,
                (z_axis.x - x_axis.z) * inv4y,
            );
        }
    } else {
        // z^2 + w^2 >= x^2 + y^2
        let sum10 = y_axis.y + x_axis.x;
        let opm22 = 1.0 + z_axis.z;
        if sum10 <= 0.0 {
            // z^2 >= w^2
            let four_zsq = opm22 - sum10;
            let inv4z = 0.5 / sqrt(four_zsq);
            return vec4<f32>(
                (x_axis.z + z_axis.x) * inv4z,
                (y_axis.z + z_axis.y) * inv4z,
                four_zsq * inv4z,
                (x_axis.y - y_axis.x) * inv4z,
            );
        } else {
            // w^2 >= z^2
            let four_wsq = opm22 + sum10;
            let inv4w = 0.5 / sqrt(four_wsq);
            return vec4<f32>(
                (y_axis.z - z_axis.y) * inv4w,
                (z_axis.x - x_axis.z) * inv4w,
                (x_axis.y - y_axis.x) * inv4w,
                four_wsq * inv4w,
            );
        }
    }
}

@compute @workgroup_size(32)
fn cull(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let instance_index = global_id.x;

    if instance_index >= instances.count {
        return;
    }

    let instance = &instances.instances[instance_index];

    let deleted = bool((*instance).packed_data >> 24);
    if deleted {
        return;
    }

    let transform = &(*instance).transform;
    let mesh_id = (*instance).packed_data & 0xFFFF;
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

    (*draw_instance).material_id = (*instance).packed_data >> 16 & 0xFF;
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
