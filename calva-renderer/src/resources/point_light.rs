use crate::util::id_generator::IdGenerator;

#[repr(C)]
#[derive(Debug, Copy, Clone, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PointLightHandle(u32);

#[repr(C)]
#[derive(Debug, Copy, Clone, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PointLight {
    pub position: glam::Vec3,
    pub radius: f32,
    pub color: glam::Vec3,
}

impl PointLight {
    pub(crate) const SIZE: wgpu::BufferAddress = std::mem::size_of::<Self>() as _;

    pub fn transform(&mut self, transform: glam::Mat4) {
        self.position = (transform * self.position.extend(1.0)).truncate();
    }

    fn address(handle: &PointLightHandle) -> wgpu::BufferAddress {
        handle.0 as wgpu::BufferAddress * Self::SIZE
    }
}

pub struct PointLightsManager {
    ids: IdGenerator,
    pub(crate) point_lights: wgpu::Buffer,
}

impl PointLightsManager {
    const MAX_POINT_LIGHTS: usize = 10_000;

    pub fn new(device: &wgpu::Device) -> Self {
        let point_lights = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("LightsManager point lights"),
            size: PointLight::SIZE * Self::MAX_POINT_LIGHTS as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            ids: IdGenerator::new(0),
            point_lights,
        }
    }

    pub fn add(
        &mut self,
        queue: &wgpu::Queue,
        point_lights: &[PointLight],
    ) -> Vec<PointLightHandle> {
        point_lights
            .iter()
            .map(|point_light| {
                let handle = PointLightHandle(self.ids.get());

                queue.write_buffer(
                    &self.point_lights,
                    PointLight::address(&handle),
                    bytemuck::bytes_of(point_light),
                );

                handle
            })
            .collect::<Vec<_>>()
    }

    pub fn remove(&mut self, queue: &wgpu::Queue, handles: &[PointLightHandle]) {
        for handle in handles {
            self.ids.recycle(handle.0);

            queue.write_buffer(
                &self.point_lights,
                PointLight::address(handle),
                bytemuck::bytes_of(&PointLight::default()),
            );
        }
    }

    pub fn count(&self) -> u32 {
        self.ids.next
    }
}

impl From<&wgpu::Device> for PointLightsManager {
    fn from(device: &wgpu::Device) -> Self {
        Self::new(device)
    }
}
