
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