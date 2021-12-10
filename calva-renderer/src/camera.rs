use wgpu::util::DeviceExt;

pub struct Camera {
    pub view: glam::Mat4,
    pub proj: glam::Mat4,

    buffer: wgpu::Buffer,
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub bind_group: wgpu::BindGroup,
}

impl Camera {
    pub const OPENGL_TO_WGPU_MATRIX: glam::Mat4 = glam::const_mat4!(
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 0.5, 0.0],
        [0.0, 0.0, 0.5, 1.0]
    );

    pub fn new(device: &wgpu::Device) -> Self {
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera buffer"),
            contents: bytemuck::cast_slice(&[glam::Mat4::default(); 5]),
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
                    min_binding_size: None,
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
            view: glam::Mat4::default(),
            proj: glam::Mat4::default(),

            buffer,
            bind_group_layout,
            bind_group,
        }
    }

    pub(crate) fn update_buffers(&self, queue: &wgpu::Queue) {
        let proj = Self::OPENGL_TO_WGPU_MATRIX * self.proj;

        queue.write_buffer(
            &self.buffer,
            0,
            bytemuck::cast_slice(&[
                self.view,
                proj,
                proj * self.view,
                self.view.inverse(),
                proj.inverse(),
            ]),
        );
    }
}
