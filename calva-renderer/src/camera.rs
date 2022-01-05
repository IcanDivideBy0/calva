use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct CameraUniformRaw {
    view: glam::Mat4,
    proj: glam::Mat4,
    view_proj: glam::Mat4,
    inv_view: glam::Mat4,
    inv_proj: glam::Mat4,
}

impl CameraUniformRaw {
    pub const OPENGL_TO_WGPU_MATRIX: glam::Mat4 = glam::const_mat4!(
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 0.5, 0.0],
        [0.0, 0.0, 0.5, 1.0]
    );

    fn new(view: glam::Mat4, proj: glam::Mat4) -> Self {
        let proj = Self::OPENGL_TO_WGPU_MATRIX * proj;

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
    const DESC: &'static wgpu::BindGroupLayoutDescriptor<'static> =
        &wgpu::BindGroupLayoutDescriptor {
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
        };

    pub fn new(device: &wgpu::Device) -> Self {
        let view = glam::Mat4::default();
        let proj = glam::Mat4::default();

        let raw = CameraUniformRaw::new(view, proj);
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera buffer"),
            contents: bytemuck::bytes_of(&raw),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group_layout = device.create_bind_group_layout(Self::DESC);

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

    pub(crate) fn update_buffer(&self, queue: &wgpu::Queue) {
        let raw = CameraUniformRaw::new(self.view, self.proj);
        queue.write_buffer(&self.buffer, 0, bytemuck::bytes_of(&raw));
    }
}
