@group(0) @binding(0) var t_depth: texture_depth_2d;
@group(0) @binding(1) var t_sampler: sampler;

@group(0) @binding(2) var t_output: texture_storage_2d<r32float, write>;

var<private> min_depth: atomic<u32>;


@compute @workgroup_size(4, 4, 1)
fn main(
  @builtin(workgroup_id) wg_id : vec3<u32>,
  @builtin(global_invocation_id) global_id: vec3<u32>,
  @builtin(local_invocation_index) local_id: u32,
) {
  if local_id == 0u {
    min_depth = bitcast<u32>(1.0);
  }

  storageBarrier();

  let base_texel_coords = 4u * global_id.xy;
  let texture_dimensions = textureDimensions(t_depth);
  let texel_dimensions = 1.0 / vec2<f32>(texture_dimensions);

  let gather_coords_1 = vec2<f32>(base_texel_coords + vec2<u32>(1u, 1u)) * texel_dimensions;
  let gather_coords_2 = vec2<f32>(base_texel_coords + vec2<u32>(3u, 1u)) * texel_dimensions;
  let gather_coords_3 = vec2<f32>(base_texel_coords + vec2<u32>(1u, 3u)) * texel_dimensions;
  let gather_coords_4 = vec2<f32>(base_texel_coords + vec2<u32>(3u, 3u)) * texel_dimensions;

  let texel_values_1 = textureGather(t_depth, t_sampler, gather_coords_1);
  let texel_values_2 = textureGather(t_depth, t_sampler, gather_coords_2);
  let texel_values_3 = textureGather(t_depth, t_sampler, gather_coords_3);
  let texel_values_4 = textureGather(t_depth, t_sampler, gather_coords_4);

  let gathered_min = min(
    min(texel_values_1, texel_values_2),
    min(texel_values_3, texel_values_4),
  );
  let final_min = min(
    min(gathered_min.x, gathered_min.y),
    min(gathered_min.z, gathered_min.w),
  );

  atomicMin(&min_depth, bitcast<u32>(final_min));

  storageBarrier();

  if local_id == 0u {
    let workgroup_min_depth = bitcast<f32>(min_depth);
    textureStore(t_output, wg_id.xy, vec4<f32>(workgroup_min_depth));
  }
}