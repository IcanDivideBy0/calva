struct Camera {
    view: mat4x4<f32>,
    proj: mat4x4<f32>,
    view_proj: mat4x4<f32>,
    inv_view: mat4x4<f32>,
    inv_proj: mat4x4<f32>,
}
@group(0) @binding(0) var<uniform> camera: Camera;

struct MeshBoundingSphere {
    center: vec3<f32>,
    radius: f32,
}

struct MeshData {
    bounding_sphere: MeshBoundingSphere,
    vertex_count: u32,
    vertex_offset: i32,
    base_index: u32,
}

struct AnimationState {
    animation_id: u32,
    time: f32,
}

struct InstanceInput {
    transform: mat4x4<f32>,
    mesh_id: u32,
    material_id: u32,
    animation: AnimationState,
}

struct InstanceOutput {
    transform: mat4x4<f32>,
    normal_quat: vec4<f32>,
    material_id: u32,
    animation: AnimationState,
}

struct DrawIndexedIndirect {
    vertex_count: u32,
    instance_count: atomic<u32>,
    base_index: u32,
    vertex_offset: i32,
    base_instance: u32,
}

@group(1) @binding(0)
var<storage, read> meshes_data: array<MeshData>;

@group(1) @binding(1)
var<storage, read> instances_input: array<InstanceInput>;
var<push_constant> instances_count: u32;

@group(1) @binding(2)
var<storage, read_write> instances_output: array<InstanceOutput>;

@group(1) @binding(3)
var<storage, read_write> indirects: array<DrawIndexedIndirect>;

@compute @workgroup_size(32)
fn init(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let mesh_id = global_id.x;

    let mesh_data = &meshes_data[mesh_id];
    let indirect = &indirects[mesh_id];

    (*indirect).vertex_count = (*mesh_data).vertex_count;
    (*indirect).base_index = (*mesh_data).base_index;
    (*indirect).vertex_offset = (*mesh_data).vertex_offset;

    atomicStore(&(*indirect).instance_count, 0u);

    (*indirect).base_instance = 0u;
    for (var i = 0u; i < instances_count; i++) {
        (*indirect).base_instance += u32(instances_input[i].mesh_id < mesh_id);
    }
}

fn normalize_plane(plane: vec4<f32>) -> vec4<f32> {
    return plane / length(plane.xyz);
}
fn plane_distance_to_point(plane: vec4<f32>, p: vec3<f32>) -> f32 {
    return dot(plane.xyz, p) + plane.w;
}
fn sphere_visible(sphere: MeshBoundingSphere, transform: mat4x4<f32>) -> bool {
    let m = transpose(camera.view_proj * transform);
    let frustum = array<vec4<f32>, 6>(
        normalize_plane(m[3] + m[0]), // left
        normalize_plane(m[3] - m[0]), // right
        normalize_plane(m[3] + m[1]), // bottom
        normalize_plane(m[3] - m[1]), // top
        normalize_plane(m[3] + m[2]), // near
        normalize_plane(m[3] - m[2]), // far
    );

    let neg_radius = -sphere.radius;
    return !(//
        plane_distance_to_point(frustum[0], sphere.center) < neg_radius || //
        plane_distance_to_point(frustum[1], sphere.center) < neg_radius || //
        plane_distance_to_point(frustum[2], sphere.center) < neg_radius || //
        plane_distance_to_point(frustum[3], sphere.center) < neg_radius || //
        plane_distance_to_point(frustum[4], sphere.center) < neg_radius || //
        plane_distance_to_point(frustum[5], sphere.center) < neg_radius);
}

fn mat3_quat(m: mat3x3<f32>) -> vec4<f32> {
    var out = vec4<f32>(0.0);

    // Algorithm in Ken Shoemake's article in 1987 SIGGRAPH course notes
    // article "Quaternion Calculus and Fast Animation".
    var fTrace = m[0].x + m[1].y + m[2].z;
    var fRoot: f32;

    if fTrace > 0.0 {
        // |w| > 1/2, may as well choose w > 1/2
        fRoot = sqrt(fTrace + 1.0); // 2w
        out.w = 0.5 * fRoot;
        fRoot = 0.5 / fRoot; // 1/(4w)
        out.x = (m[1].z - m[2].y) * fRoot;
        out.y = (m[2].x - m[0].z) * fRoot;
        out.z = (m[0].y - m[1].x) * fRoot;
    } else {
        // |w| <= 1/2
        if m[1].y > m[0].x {
            if m[2].z > m[1][1] {
                fRoot = sqrt(m[2][2] - m[0][0] - m[1][1] + 1.0);
                out[2] = 0.5 * fRoot;
                fRoot = 0.5 / fRoot;
                out[3] = (m[0][1] - m[1][0]) * fRoot;
                out[0] = (m[0][2] + m[2][0]) * fRoot;
                out[1] = (m[1][2] + m[2][1]) * fRoot;
            } else {
                fRoot = sqrt(m[1][1] - m[2][2] - m[0][0] + 1.0);
                out[1] = 0.5 * fRoot;
                fRoot = 0.5 / fRoot;
                out[3] = (m[2][0] - m[0][2]) * fRoot;
                out[2] = (m[2][1] + m[1][2]) * fRoot;
                out[0] = (m[0][1] + m[1][0]) * fRoot;
            }
        } else {
            fRoot = sqrt(m[0][0] - m[1][1] - m[2][2] + 1.0);
            out[0] = 0.5 * fRoot;
            fRoot = 0.5 / fRoot;
            out[3] = (m[1][2] - m[2][1]) * fRoot;
            out[1] = (m[1][0] + m[0][1]) * fRoot;
            out[2] = (m[2][0] + m[0][2]) * fRoot;
        }
    }
    return normalize(out);
}

@compute @workgroup_size(32)
fn cull(@builtin(global_invocation_id) global_id: vec3<u32>) {
    if global_id.x >= instances_count {
        return;
    }

    let instance_input = &instances_input[global_id.x];
    let transform = &(*instance_input).transform;
    let mesh_data = &meshes_data[(*instance_input).mesh_id];

    if !sphere_visible((*mesh_data).bounding_sphere, (*transform)) {
        return;
    }

    let normal_quat = mat3_quat(mat3x3<f32>(
        (*transform)[0].xyz,
        (*transform)[1].xyz,
        (*transform)[2].xyz,
    ));

    let indirect = &indirects[(*instance_input).mesh_id];
    let instance_index = (*indirect).base_instance + atomicAdd(&(*indirect).instance_count, 1u);

    var out: InstanceOutput;
    out.transform = *transform;
    out.normal_quat = normal_quat;
    out.material_id = (*instance_input).material_id;
    out.animation = (*instance_input).animation;

    instances_output[instance_index] = out;
}
