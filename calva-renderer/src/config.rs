use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct RendererConfigData {
    pub ssao_radius: f32,
    pub ssao_bias: f32,
    pub ssao_power: f32,
    pub ambient_factor: f32,
    pub shadow_variance_min: f32,
    pub shadow_light_bleed_reduction: f32,
}

impl RendererConfigData {
    fn default() -> Self {
        Self {
            ssao_radius: 0.3,
            ssao_bias: 0.025,
            ssao_power: 2.0,
            ambient_factor: 0.1,
            shadow_variance_min: 0.0002,
            shadow_light_bleed_reduction: 0.6,
        }
    }
}

pub struct RendererConfig {
    pub data: RendererConfigData,

    pub buffer: wgpu::Buffer,
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub bind_group: wgpu::BindGroup,
}

impl RendererConfig {
    pub fn new(device: &wgpu::Device) -> Self {
        let data = RendererConfigData::default();

        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Renderer config buffer"),
            contents: bytemuck::cast_slice(&[data]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Renderer config bind group layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::all(),
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Renderer config bind group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        });

        Self {
            data,

            buffer,
            bind_group_layout,
            bind_group,
        }
    }

    pub(crate) fn update_buffer(&self, queue: &wgpu::Queue) {
        queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(&[self.data]));
    }
}

impl std::ops::Deref for RendererConfig {
    type Target = RendererConfigData;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}
