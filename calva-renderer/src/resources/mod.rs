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
mod time;

pub use animation::*;
pub use camera::*;
pub use material::*;
pub use mesh::*;
pub use mesh_instance::*;
pub use mipmap::*;
pub use point_light::*;
pub use skin::*;
pub use skybox::*;
pub use texture::*;
pub use time::*;

use anyhow::Result;
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
    fn instanciate(resources: &ResourcesManager) -> Result<Self>
    where
        Self: Sized;

    fn update(&mut self, _resources: &ResourcesManager) -> Result<()> {
        Ok(())
    }

    fn update_dependencies() -> impl IntoIterator<Item = TypeId>
    where
        Self: Sized,
    {
        []
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
    resources: Arc<RwLock<HashMap<TypeId, ResourceArc>>>,
    call_stack: Arc<RwLock<Vec<(TypeId, String)>>>,
}

impl ResourcesManager {
    pub(crate) fn new(
        device: wgpu::Device,
        queue: wgpu::Queue,
        surface: wgpu::Surface<'static>,
        surface_config: wgpu::SurfaceConfiguration,
    ) -> Self {
        let mut resources: HashMap<TypeId, ResourceArc> = HashMap::new();

        resources.insert(
            TypeId::of::<wgpu::Device>(),
            Arc::new(RwLock::new(Box::new(device))),
        );

        resources.insert(
            TypeId::of::<wgpu::Queue>(),
            Arc::new(RwLock::new(Box::new(queue))),
        );

        resources.insert(
            TypeId::of::<wgpu::Surface>(),
            Arc::new(RwLock::new(Box::new(surface))),
        );

        resources.insert(
            TypeId::of::<wgpu::SurfaceConfiguration>(),
            Arc::new(RwLock::new(Box::new(surface_config))),
        );

        Self {
            resources: Arc::new(RwLock::new(resources)),
            call_stack: Default::default(),
        }
    }

    pub(crate) fn update(&self) -> Result<()> {
        let arcs = {
            // Some updates might read resources that are not created yet.
            // That's why it's important not to lock the resources registry
            // during the update loop.
            let read = self.resources.read();
            read.values().cloned().collect::<Vec<_>>()
        };

        for arc in arcs {
            // TODO: check update_dependencies before updating
            arc.write_arc().update(self)?;
        }

        Ok(())
    }

    fn instanciate<T: Resource>(&self) -> Result<T> {
        let ty_id = TypeId::of::<T>();
        let ty_name = type_name::<T>().to_string();

        {
            let mut stack = self.call_stack.write();

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

        self.call_stack.write().pop();

        resource
    }

    fn get_arc<T: Resource>(&self) -> ResourceArc {
        let read = self.resources.read();

        match read.get(&TypeId::of::<T>()) {
            Some(res) => res.clone(),
            None => {
                drop(read); // prevent deadlock

                let resource = self.instanciate::<T>().unwrap();

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
    fn instanciate(_resources: &ResourcesManager) -> Result<Self> {
        unreachable!()
    }
}

impl Resource for wgpu::Queue {
    fn instanciate(_resources: &ResourcesManager) -> Result<Self> {
        unreachable!()
    }
}

impl Resource for wgpu::Surface<'static> {
    fn instanciate(_resources: &ResourcesManager) -> Result<Self> {
        unreachable!()
    }

    fn update(&mut self, resources: &ResourcesManager) -> Result<()> {
        let device = resources.read::<wgpu::Device>();
        let surface_config = resources.read::<wgpu::SurfaceConfiguration>();

        match self.get_configuration() {
            Some(ref current_config) => {
                if &*surface_config != current_config {
                    self.configure(&device, &surface_config);
                }
            }
            None => unreachable!(),
        }

        Ok(())
    }
}

impl Resource for wgpu::SurfaceConfiguration {
    fn instanciate(_resources: &ResourcesManager) -> Result<Self> {
        unreachable!()
    }
}
