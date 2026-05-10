mod animation;
mod camera;
mod material;
mod mesh;
mod mesh_instance;
mod mipmap;
mod point_light;
mod skin;
mod skybox;
mod texture;

pub use animation::*;
use anyhow::Result;
pub use camera::*;
pub use material::*;
pub use mesh::*;
pub use mesh_instance::*;
pub use mipmap::*;
pub use point_light::*;
pub use skin::*;
pub use skybox::*;
pub use texture::*;

use downcast_rs::{impl_downcast, DowncastSend, DowncastSync};
use parking_lot::{ArcRwLockReadGuard, ArcRwLockWriteGuard, RawRwLock, RwLock};
use std::{
    any::{type_name, TypeId},
    collections::HashMap,
    marker::PhantomData,
    ops::{Deref, DerefMut},
    sync::Arc,
};

pub trait Resource: DowncastSync + DowncastSend {
    fn instanciate(resources: &ResourcesManager) -> Self
    where
        Self: Sized;

    fn update(&mut self, _resources: &ResourcesManager) -> Result<()> {
        Ok(())
    }
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

type ResourceArc = Arc<RwLock<Box<dyn Resource>>>;

#[derive(Clone)]
pub struct ResourcesManager {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface_config: Arc<RwLock<wgpu::SurfaceConfiguration>>,

    resources: Arc<RwLock<HashMap<TypeId, ResourceArc>>>,
    instantiation_stack: Arc<RwLock<Vec<(TypeId, String)>>>,
}

impl ResourcesManager {
    pub(crate) fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        surface_config: &wgpu::SurfaceConfiguration,
    ) -> Self {
        Self {
            device: device.clone(),
            queue: queue.clone(),
            surface_config: Arc::new(RwLock::new(surface_config.clone())),

            resources: Default::default(),
            instantiation_stack: Default::default(),
        }
    }

    pub(crate) fn update(&self) -> Result<()> {
        for arc in self.resources.read().values() {
            arc.write_arc().update(self)?;
        }

        Ok(())
    }

    fn instanciate<T: Resource>(&self) -> T {
        let ty_id = TypeId::of::<T>();
        let ty_name = type_name::<T>().to_string();

        {
            let mut stack = self.instantiation_stack.write();

            if stack
                .iter()
                .find(|(type_id, _)| *type_id == ty_id)
                .is_some()
            {
                panic!(
                    "Recursion detected in resources instantiation:{}",
                    stack
                        .iter()
                        .map(|(_, name)| name)
                        .chain(std::iter::once(&ty_name))
                        .map(|name| format!("\n  -> {}", name))
                        .collect::<String>()
                );
            }

            stack.push((ty_id, ty_name));
        }

        let resource = T::instanciate(self);

        self.instantiation_stack.write().pop();

        resource
    }

    fn get_arc<T: Resource>(&self) -> ResourceArc {
        let read = self.resources.read();

        match read.get(&TypeId::of::<T>()) {
            Some(res) => res.clone(),
            None => {
                drop(read); // prevent deadlock

                let resource = self.instanciate::<T>();

                self.resources
                    .write()
                    .entry(TypeId::of::<T>())
                    .or_insert_with(|| Arc::new(RwLock::new(Box::new(resource))))
                    .clone()
            }
        }
    }

    pub fn read<T: Resource>(&self) -> ResourceReadLock<T> {
        ResourceReadLock(self.get_arc::<T>().read_arc(), PhantomData)
    }

    pub fn write<T: Resource>(&self) -> ResourceWriteLock<T> {
        ResourceWriteLock(self.get_arc::<T>().write_arc(), PhantomData)
    }
}

impl Resource for wgpu::Device {
    fn instanciate(resources: &ResourcesManager) -> Self {
        resources.device.clone()
    }
}

impl Resource for wgpu::Queue {
    fn instanciate(resources: &ResourcesManager) -> Self {
        resources.queue.clone()
    }
}

impl Resource for wgpu::SurfaceConfiguration {
    fn instanciate(resources: &ResourcesManager) -> Self {
        resources.surface_config.read_arc().clone()
    }
}
