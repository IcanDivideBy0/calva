use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct GpuCamera {
    view: glam::Mat4,
    proj: glam::Mat4,
    view_proj: glam::Mat4,
    inv_view: glam::Mat4,
    inv_proj: glam::Mat4,
}

impl GpuCamera {
    fn new(view: glam::Mat4, proj: glam::Mat4) -> Self {
        Self {
            view,
            proj,
            view_proj: proj * view,
            inv_view: view.inverse(),
            inv_proj: proj.inverse(),
        }
    }
}

pub struct CameraUniform {
    pub view: glam::Mat4,
    pub proj: glam::Mat4,

    buffer: wgpu::Buffer,
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub bind_group: wgpu::BindGroup,
}

impl CameraUniform {
    pub fn new(device: &wgpu::Device) -> Self {
        let view = glam::Mat4::default();
        let proj = glam::Mat4::default();

        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera buffer"),
            contents: bytemuck::bytes_of(&GpuCamera::new(view, proj)),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Camera bind group layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::all(),
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(std::mem::size_of::<GpuCamera>() as _),
                },
                count: None,
            }],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Camera bind group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        });

        Self {
            view,
            proj,

            buffer,
            bind_group_layout,
            bind_group,
        }
    }

    pub fn update(&mut self, queue: &wgpu::Queue, view: glam::Mat4, proj: glam::Mat4) {
        self.view = view;
        self.proj = proj;

        queue.write_buffer(
            &self.buffer,
            0,
            bytemuck::bytes_of(&GpuCamera::new(self.view, self.proj)),
        );
    }
}
