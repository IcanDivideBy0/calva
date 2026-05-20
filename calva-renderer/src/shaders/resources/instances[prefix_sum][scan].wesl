@binding(0) @group(0) var<storage, read> input: array<f32>;
@binding(1) @group(0) var<storage, read_write> output: array<f32>;
@binding(2) @group(0) var<uniform> n: u32;

const bank_size: u32 = 32;
var<workgroup> temp: array<f32, 532>; // why 532??

fn bank_conflict_free_idx(idx: u32) -> u32 {
    var chunk_id: u32 = idx / bank_size;
    return idx + chunk_id;
}

@compute @workgroup_size(256)
fn main(
    @builtin(global_invocation_id) GlobalInvocationID: vec3<u32>,
    @builtin(local_invocation_id) LocalInvocationId: vec3<u32>,
    @builtin(workgroup_id) WorkgroupID: vec3<u32>
) {
    var thread_id: u32 = LocalInvocationId.x;
    var global_thread_id: u32 = GlobalInvocationID.x;

    if (thread_id < (n>>1)) {
        temp[bank_conflict_free_idx(2*thread_id)] = input[2*global_thread_id];
        temp[bank_conflict_free_idx(2*thread_id + 1)] = input[2*global_thread_id + 1];
    }

    workgroupBarrier();
    var offset: u32 = 1;

    for (var d:u32 = n>>1; d > 0; d >>= 1) {
        if (thread_id < d) {
            var ai: u32 = offset * (2 * thread_id + 1) - 1;
            var bi: u32 = offset * (2 * thread_id + 2) - 1;
            temp[bank_conflict_free_idx(bi)] += temp[bank_conflict_free_idx(ai)];
        }
        offset *= 2;
        workgroupBarrier();
    }

    if (thread_id == 0) {
       temp[bank_conflict_free_idx(n - 1)] = 0.0;
    }
    workgroupBarrier();

    for (var d: u32 = 1; d < n; d *= 2) {
        offset >>= 1;
        if (thread_id < d) {
            var ai: u32 = offset * (2 * thread_id + 1) - 1;
            var bi: u32 = offset * (2 * thread_id + 2) - 1;
            var t: f32 = temp[bank_conflict_free_idx(ai)];
            temp[bank_conflict_free_idx(ai)] = temp[bank_conflict_free_idx(bi)];
            temp[bank_conflict_free_idx(bi)] += t;
        }
        workgroupBarrier();
    }

    if (thread_id < (n>>1)) {
        output[2*global_thread_id] = temp[bank_conflict_free_idx(2*thread_id)];
        output[2*global_thread_id + 1] = temp[bank_conflict_free_idx(2*thread_id + 1)];
    }
}