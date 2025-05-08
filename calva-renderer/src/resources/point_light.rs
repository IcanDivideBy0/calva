use crate::util::id_generator::IdGenerator;

#[repr(C)]
#[derive(
    Debug, Copy, Clone, Default, PartialEq, Eq, Ord, PartialOrd, bytemuck::Pod, bytemuck::Zeroable,
)]
pub struct PointLightHandle(u16);

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
    const MAX_POINT_LIGHTS: usize = 1 << 16;

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

    pub fn count(&self) -> u16 {
        self.ids.count()
    }

    pub fn add(
        &mut self,
        queue: &wgpu::Queue,
        point_lights: &[PointLight],
    ) -> Vec<PointLightHandle> {
        let handles = point_lights
            .iter()
            .map(|_| PointLightHandle(self.ids.get()))
            .collect::<Vec<_>>();

        let mut writes: Vec<(wgpu::BufferAddress, Vec<PointLight>)> = vec![];
        if let Some((handle, point_light)) = Option::zip(handles.first(), point_lights.first()) {
            writes.push((PointLight::address(handle), vec![*point_light]));
        } else {
            return handles;
        }

        for (idx, pair) in handles.windows(2).enumerate() {
            let prev = pair[0];
            let next = pair[1];

            if next.0 != prev.0 + 1 {
                writes.push((PointLight::address(&next), vec![]));
            }

            writes.last_mut().unwrap().1.push(point_lights[idx + 1]);
        }

        for (address, point_lights) in writes {
            queue.write_buffer(
                &self.point_lights,
                address,
                bytemuck::cast_slice(&point_lights),
            );
        }

        handles
    }

    pub fn remove(&mut self, queue: &wgpu::Queue, handles: &mut [PointLightHandle]) {
        handles.sort();

        let mut writes: Vec<(wgpu::BufferAddress, Vec<PointLight>)> = vec![];
        if let Some(handle) = handles.first() {
            self.ids.recycle(handle.0 as _);
            writes.push((PointLight::address(handle), vec![PointLight::default()]));
        } else {
            return;
        }

        for pair in handles.windows(2) {
            let prev = pair[0];
            let next = pair[1];

            self.ids.recycle(next.0 as _);
            if next.0 != prev.0 + 1 {
                writes.push((PointLight::address(&next), vec![]));
            }

            writes.last_mut().unwrap().1.push(PointLight::default());
        }

        for (address, point_lights) in writes {
            queue.write_buffer(
                &self.point_lights,
                address,
                bytemuck::cast_slice(&point_lights),
            );
        }
    }
}

impl From<&wgpu::Device> for PointLightsManager {
    fn from(device: &wgpu::Device) -> Self {
        Self::new(device)
    }
}
