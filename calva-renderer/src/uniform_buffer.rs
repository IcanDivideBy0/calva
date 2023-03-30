use wgpu::util::DeviceExt;

pub trait UniformData {
    type GpuType: bytemuck::NoUninit;

    fn as_gpu_type(&self) -> Self::GpuType;
}

impl<T: Copy + bytemuck::NoUninit> UniformData for T {
    type GpuType = Self;

    fn as_gpu_type(&self) -> Self::GpuType {
        *self
    }
}

pub struct UniformBuffer<T> {
    cpu: T,
    gpu: T,

    pub buffer: wgpu::Buffer,
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub bind_group: wgpu::BindGroup,
}

impl<T: Copy + PartialEq + UniformData> UniformBuffer<T> {
    pub fn new(device: &wgpu::Device, value: T) -> Self {
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("Uniform buffer: {}", std::any::type_name::<T>())),
            contents: bytemuck::bytes_of(&value.as_gpu_type()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some(&format!(
                "Uniform bind group layout: {}",
                std::any::type_name::<T>()
            )),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::all(),
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(0 as _),
                },
                count: None,
            }],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(&format!(
                "Uniform bind group: {}",
                std::any::type_name::<T>()
            )),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        });

        Self {
            cpu: value,
            gpu: value,

            buffer,
            bind_group_layout,
            bind_group,
        }
    }

    pub fn update(&mut self, queue: &wgpu::Queue) {
        if self.gpu != self.cpu {
            self.gpu = self.cpu;
            queue.write_buffer(&self.buffer, 0, bytemuck::bytes_of(&self.gpu.as_gpu_type()));
        }
    }
}

impl<T> std::ops::Deref for UniformBuffer<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.cpu
    }
}

impl<T> std::ops::DerefMut for UniformBuffer<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.cpu
    }
}
