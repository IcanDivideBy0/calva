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

use downcast_rs::{impl_downcast, DowncastSend, DowncastSync};
use parking_lot::{ArcRwLockReadGuard, ArcRwLockWriteGuard, RawRwLock, RwLock};
use std::{
    any::TypeId,
    collections::HashMap,
    marker::PhantomData,
    ops::{Deref, DerefMut},
    sync::Arc,
};

pub trait Resource: DowncastSync + DowncastSend {
    fn instanciate(device: &wgpu::Device, queue: &wgpu::Queue) -> Self
    where
        Self: Sized;

    fn update(&mut self) {}
}
impl_downcast!(sync Resource);

pub struct ResourceReadLock<T: Resource>(
    ArcRwLockReadGuard<RawRwLock, Box<dyn Resource>>,
    PhantomData<T>,
);

impl<T: Resource> Deref for ResourceReadLock<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.0.downcast_ref::<T>().unwrap()
    }
}

pub struct ResourceWriteLock<T: Resource>(
    ArcRwLockWriteGuard<RawRwLock, Box<dyn Resource>>,
    PhantomData<T>,
);

impl<T: Resource> Deref for ResourceWriteLock<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.0.downcast_ref::<T>().unwrap()
    }
}

impl<T: Resource> DerefMut for ResourceWriteLock<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.downcast_mut::<T>().unwrap()
    }
}

#[derive(Clone)]
pub struct ResourcesManager {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    // resources: Arc<RwLock<HashMap<TypeId, Arc<dyn Any + Send + Sync>>>>,
    #[allow(clippy::type_complexity)]
    resources: Arc<RwLock<HashMap<TypeId, Arc<RwLock<Box<dyn Resource>>>>>>,
}

impl ResourcesManager {
    pub(crate) fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        Self {
            device: device.clone(),
            queue: queue.clone(),
            resources: Default::default(),
        }
    }

    pub fn update(&self) {
        for (_ty_id, arc) in self.resources.write().iter() {
            arc.write_arc().update();
        }
    }

    fn get_arc<T: Resource>(&self) -> Arc<RwLock<Box<dyn Resource>>> {
        let mut read = self.resources.upgradable_read();

        read.get(&TypeId::of::<T>()).cloned().unwrap_or_else(|| {
            read.with_upgraded(|resources| {
                resources
                    .entry(TypeId::of::<T>())
                    .or_insert_with(|| {
                        let resource = <T as Resource>::instanciate(&self.device, &self.queue);
                        Arc::new(RwLock::new(Box::new(resource)))
                    })
                    .clone()
            })
        })
    }

    pub fn read<T: Resource>(&self) -> ResourceReadLock<T> {
        ResourceReadLock(self.get_arc::<T>().read_arc(), PhantomData)
    }

    pub fn write<T: Resource>(&self) -> ResourceWriteLock<T> {
        ResourceWriteLock(self.get_arc::<T>().write_arc(), PhantomData)
    }
}
