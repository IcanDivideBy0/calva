mod animation;
mod camera;
mod material;
mod mesh;
mod mesh_instance;
mod point_light;
mod skin;
mod skybox;
mod texture;

pub use animation::*;
pub use camera::*;
pub use material::*;
pub use mesh::*;
pub use mesh_instance::*;
pub use point_light::*;
pub use skin::*;
pub use skybox::*;
pub use texture::*;

use parking_lot::RwLock;
use std::{
    any::{Any, TypeId},
    collections::HashMap,
    ops::{Deref, DerefMut},
    sync::Arc,
};

pub trait Resource: Send + Sync + 'static {
    fn instanciate(device: &wgpu::Device, queue: &wgpu::Queue) -> Self;
}

#[derive(Clone)]
pub struct ResourceRef<T: Resource>(Arc<RwLock<T>>);

impl<T: Resource> ResourceRef<T> {
    pub fn get(&self) -> impl Deref<Target = T> + '_ {
        self.0.as_ref().read()
    }

    pub fn get_mut(&self) -> impl DerefMut<Target = T> + '_ {
        self.0.as_ref().write()
    }
}

#[derive(Clone)]
pub struct ResourcesManager {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    resources: Arc<RwLock<HashMap<TypeId, Arc<dyn Any + Send + Sync>>>>,
}

impl ResourcesManager {
    pub(crate) fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        Self {
            device: device.clone(),
            queue: queue.clone(),
            resources: Default::default(),
        }
    }

    pub fn get<T: Resource>(&self) -> ResourceRef<T> {
        let read = self.resources.read();

        let arc = match read.get(&TypeId::of::<T>()) {
            Some(arc) => arc.clone(),
            None => {
                drop(read); // prevent deadlock

                self.resources
                    .write()
                    .entry(TypeId::of::<T>())
                    .or_insert_with(|| {
                        let resource = <T as Resource>::instanciate(&self.device, &self.queue);
                        Arc::new(RwLock::new(resource))
                    })
                    .clone()
            }
        };

        ResourceRef(arc.downcast::<RwLock<T>>().unwrap())
    }
}
