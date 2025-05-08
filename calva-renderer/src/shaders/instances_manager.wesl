import package::resources::instances::{Instances, instance_id};

@group(0) @binding(0)
var<storage, read> updates: Instances;

@group(0) @binding(1)
var<storage, read_write> instances: Instances;

@compute @workgroup_size(32)
fn maintain(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let update_index = global_id.x;

    if update_index >= updates.count {
        return;
    }

    let update = &updates.instances[update_index];
    let instance_index = instance_id(*update);

    instances.instances[instance_index] = *update;
}
