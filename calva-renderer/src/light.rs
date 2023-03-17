use std::sync::atomic::{AtomicU32, Ordering};

#[repr(C)]
#[derive(Debug, Copy, Clone, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PointLightId(u32);

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
}

pub struct DirectionalLight {
    pub direction: glam::Vec3,
    pub color: glam::Vec4,
}

impl DirectionalLight {}

pub struct LightsManager {
    point_light_index: AtomicU32,
    pub(crate) point_lights: wgpu::Buffer,
}

impl LightsManager {
    const MAX_POINT_LIGHTS: usize = 10_000;

    pub fn new(device: &wgpu::Device) -> Self {
        let point_lights = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("LightsManager point lights"),
            size: PointLight::SIZE * Self::MAX_POINT_LIGHTS as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            point_light_index: AtomicU32::new(0),
            point_lights,
        }
    }

    pub fn count_point_lights(&self) -> u32 {
        self.point_light_index.load(Ordering::Relaxed)
    }

    pub fn add_point_lights(
        &mut self,
        queue: &wgpu::Queue,
        point_lights: &[PointLight],
    ) -> Vec<PointLightId> {
        let point_light_index = self
            .point_light_index
            .fetch_add(point_lights.len() as _, Ordering::Relaxed);

        queue.write_buffer(
            &self.point_lights,
            point_light_index as wgpu::BufferAddress * PointLight::SIZE,
            bytemuck::cast_slice(point_lights),
        );

        (0_u32..point_lights.len() as _)
            .map(|i| PointLightId(point_light_index + i))
            .collect()
    }
}
