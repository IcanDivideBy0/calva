mod animation;
mod camera;
mod instance;
mod light;
mod material;
mod mesh;
mod skin;
mod skybox;
mod texture;

pub use animation::*;
pub use camera::*;
pub use instance::*;
pub use light::*;
pub use material::*;
pub use mesh::*;
pub use skin::*;
pub use skybox::*;
pub use texture::*;

use parking_lot::RwLock;
use std::{
    any::{Any, TypeId},
    collections::HashMap,
    sync::Arc,
};

pub trait Ressource: Sized {
    fn instanciate(device: &wgpu::Device) -> Self;
}

#[derive(Clone)]
pub struct RessourceRef<T>(Arc<RwLock<T>>);

impl<T: Ressource> RessourceRef<T> {
    pub fn get(&self) -> impl std::ops::Deref<Target = T> + '_ {
        self.0.as_ref().read()
    }

    pub fn get_mut(&self) -> impl std::ops::DerefMut<Target = T> + '_ {
        self.0.as_ref().write()
    }
}

pub struct RessourcesManager {
    device: Arc<wgpu::Device>,
    ressources: RwLock<HashMap<TypeId, Arc<dyn Any + Send + Sync>>>,
}

impl RessourcesManager {
    pub fn new(device: Arc<wgpu::Device>) -> Self {
        Self {
            device,
            ressources: Default::default(),
        }
    }

    pub fn get<T>(&self) -> RessourceRef<T>
    where
        T: Ressource + Send + Sync + 'static,
    {
        let read = self.ressources.read();

        let arc = match read.get(&TypeId::of::<T>()) {
            Some(arc) => arc.clone(),
            None => {
                drop(read); // prevent deadlock

                self.ressources
                    .write()
                    .entry(TypeId::of::<T>())
                    .or_insert_with(|| {
                        let ressource = <T as Ressource>::instanciate(&self.device);
                        Arc::new(RwLock::new(ressource))
                    })
                    .clone()
            }
        };

        RessourceRef(arc.downcast::<RwLock<T>>().unwrap())
    }
}
