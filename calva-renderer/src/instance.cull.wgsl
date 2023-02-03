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

struct CullInstance {
    transform: mat4x4<f32>,
    mesh_id: u32,
    material_id: u32,
    animation: AnimationState,
}

struct MeshInstance {
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
struct IndirectsBuffer {
    count: atomic<u32>,
    draws: array<DrawIndexedIndirect>,
}

@group(0) @binding(0)
var<uniform> frustum: array<vec4<f32>, 6>;

@group(0) @binding(1)
var<storage, read> meshes_info: array<MeshInfo>;

@group(0) @binding(2)
var<storage, read> base_instances: array<u32>;

@group(0) @binding(3)
var<storage, read> cull_instances: array<CullInstance>;
var<push_constant> cull_instances_count: u32;

@group(0) @binding(4)
var<storage, read_write> mesh_instances: array<MeshInstance>;

@group(0) @binding(5)
var<storage, read_write> indirects: IndirectsBuffer;

@compute @workgroup_size(32)
fn reset(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let mesh_id = global_id.x;
    let mesh_info = &meshes_info[mesh_id];
    let draw = &indirects.draws[mesh_id];

    (*draw).vertex_count = (*mesh_info).vertex_count;
    (*draw).instance_count = 0u;
    (*draw).base_index = (*mesh_info).base_index;
    (*draw).vertex_offset = (*mesh_info).vertex_offset;
    (*draw).base_instance = base_instances[mesh_id];

    indirects.count = 0u;
}

fn plane_distance_to_point(plane: vec4<f32>, p: vec3<f32>) -> f32 {
    return dot(plane.xyz, p) + plane.w;
}
fn sphere_visible(sphere: MeshBoundingSphere, transform: mat4x4<f32>) -> bool {
    let p = transform * vec4<f32>(sphere.center, 1.0);
    let pos = p.xyz / p.w;

    let neg_radius = -sphere.radius;
    return !(
        plane_distance_to_point(frustum[0], pos) < neg_radius ||
        plane_distance_to_point(frustum[1], pos) < neg_radius ||
        plane_distance_to_point(frustum[2], pos) < neg_radius ||
        plane_distance_to_point(frustum[3], pos) < neg_radius ||
        plane_distance_to_point(frustum[4], pos) < neg_radius ||
        plane_distance_to_point(frustum[5], pos) < neg_radius
    );
}

fn mat_quat(m: mat4x4<f32>) -> vec4<f32> {
    let inv_scale = 1.0 / vec3<f32>(
        length(m[0].xyz) * sign(determinant(m)),
        length(m[1].xyz),
        length(m[2].xyz),
    );

    let x_axis = m[0].xyz * inv_scale.x;
    let y_axis = m[1].xyz * inv_scale.y;
    let z_axis = m[2].xyz * inv_scale.z;
    
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
    let cull_instance_index = global_id.x;

    if cull_instance_index >= cull_instances_count {
        return;
    }

    let cull_instance = &cull_instances[cull_instance_index];
    let transform = &(*cull_instance).transform;
    let mesh_info = &meshes_info[(*cull_instance).mesh_id];

    if !sphere_visible((*mesh_info).bounding_sphere, (*transform)) {
        return;
    }

    let draw = &indirects.draws[(*cull_instance).mesh_id];
    let mesh_instance_index = (*draw).base_instance + atomicAdd(&(*draw).instance_count, 1u);

    let mesh_instance = &mesh_instances[mesh_instance_index];
    (*mesh_instance).transform = *transform;
    (*mesh_instance).normal_quat = mat_quat(*transform);
    (*mesh_instance).material_id = (*cull_instance).material_id;
    (*mesh_instance).skin_offset = (*mesh_info).skin_offset;
    (*mesh_instance).animation = (*cull_instance).animation;
}

@compute @workgroup_size(32)
fn count(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let mesh_id = global_id.x;

    let draw = &indirects.draws[mesh_id];
    if (*draw).instance_count > 0u {
        indirects.draws[atomicAdd(&indirects.count, 1u)] = *draw;
    }
}
