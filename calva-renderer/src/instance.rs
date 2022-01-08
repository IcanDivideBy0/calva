use wgpu::util::DeviceExt;

pub trait Instance: bytemuck::Pod {
    const SIZE: usize;
    const LAYOUT: wgpu::VertexBufferLayout<'static>;
}

pub struct Instances<T: Instance> {
    instances: Vec<T>,
    pub buffer: wgpu::Buffer,
}

impl<T: Instance> Instances<T> {
    pub const MAX_INSTANCES: usize = 100;

    pub fn new(device: &wgpu::Device) -> Self {
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Instances Buffer"),
            contents: &vec![0u8; T::SIZE * Self::MAX_INSTANCES],
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });

        Self {
            instances: Vec::with_capacity(Self::MAX_INSTANCES),
            buffer,
        }
    }

    pub fn count(&self) -> u32 {
        self.instances.len() as u32
    }

    pub fn write_buffer(&self, queue: &wgpu::Queue) {
        queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(&self.instances));
    }
}

impl<T: Instance> std::ops::Deref for Instances<T> {
    type Target = Vec<T>;

    fn deref(&self) -> &Self::Target {
        &self.instances
    }
}

impl<T: Instance> std::ops::DerefMut for Instances<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.instances
    }
}
