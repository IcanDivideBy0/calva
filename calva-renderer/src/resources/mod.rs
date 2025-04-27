mod animation;
mod camera;
mod instance;
mod material;
mod mesh;
mod point_light;
mod skin;
mod skybox;
mod texture;

pub use animation::*;
pub use camera::*;
pub use instance::*;
pub use material::*;
pub use mesh::*;
pub use point_light::*;
pub use skin::*;
pub use skybox::*;
pub use texture::*;

use parking_lot::RwLock;
use std::{
    any::{Any, TypeId},
    collections::HashMap,
    sync::Arc,
};

#[derive(Clone)]
pub struct ResourceRef<T>(Arc<RwLock<T>>);

impl<T> ResourceRef<T>
where
    T: for<'a> From<&'a wgpu::Device>,
{
    pub fn get(&self) -> impl std::ops::Deref<Target = T> + '_ {
        self.0.as_ref().read()
    }

    pub fn get_mut(&self) -> impl std::ops::DerefMut<Target = T> + '_ {
        self.0.as_ref().write()
    }
}

pub struct ResourcesManager {
    device: wgpu::Device,
    resources: RwLock<HashMap<TypeId, Arc<dyn Any + Send + Sync>>>,
}

impl ResourcesManager {
    pub fn new(device: &wgpu::Device) -> Self {
        Self {
            device: device.clone(),
            resources: Default::default(),
        }
    }

    pub fn get<T>(&self) -> ResourceRef<T>
    where
        T: for<'a> From<&'a wgpu::Device> + Send + Sync + 'static,
    {
        let read = self.resources.read();

        let arc = match read.get(&TypeId::of::<T>()) {
            Some(arc) => arc.clone(),
            None => {
                drop(read); // prevent deadlock

                self.resources
                    .write()
                    .entry(TypeId::of::<T>())
                    .or_insert_with(|| {
                        let resource = <T as From<&wgpu::Device>>::from(&self.device);
                        Arc::new(RwLock::new(resource))
                    })
                    .clone()
            }
        };

        ResourceRef(arc.downcast::<RwLock<T>>().unwrap())
    }
}
