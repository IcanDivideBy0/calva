#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TimestampData {
    start: u64,
    end: u64,
}

impl TimestampData {
    const SIZE: wgpu::BufferAddress = std::mem::size_of::<Self>() as _;
}

pub struct Timers {
    period: f32,
    buffer: wgpu::Buffer,
    queries: wgpu::QuerySet,
}

impl Timers {
    const BUFFER_SIZE: wgpu::BufferAddress = wgpu::QUERY_RESOLVE_BUFFER_ALIGNMENT;
    const TIMERS_COUNT: u32 = (Self::BUFFER_SIZE / TimestampData::SIZE) as u32;

    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let period = queue.get_timestamp_period();

        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("TimedPasses buffer"),
            size: Self::BUFFER_SIZE,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let queries = device.create_query_set(&wgpu::QuerySetDescriptor {
            label: None,
            ty: wgpu::QueryType::Timestamp,
            count: Self::TIMERS_COUNT,
        });

        Self {
            period,
            buffer,
            queries,
        }
    }

    pub fn resolve(&self, encoder: &mut wgpu::CommandEncoder) {
        encoder.resolve_query_set(&self.queries, 0..Self::TIMERS_COUNT, &self.buffer, 0);
    }

    pub fn finish(&self, device: &wgpu::Device) {
        let _ = self.buffer.slice(..).map_async(wgpu::MapMode::Read);
        device.poll(wgpu::Maintain::Wait);

        let view = self.buffer.slice(..).get_mapped_range();

        let timers: &[TimestampData] = bytemuck::cast_slice(&*view);
        dbg!(timers);

        self.buffer.unmap();

        // let nanoseconds = (timestamp_data[1] - timestamp_data[0]) as f32 * timestamp_period;
        // let microseconds = nanoseconds / 1000.0;

        // println!("PointLights lighting pass: {:.3} μs", microseconds);
    }
}
