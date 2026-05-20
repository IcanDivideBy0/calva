use crate::{
    AnimationState, MeshInstance, MeshInstanceFlags, MeshInstanceHandle, MeshInstancesManager,
    PointLight, PointLightHandle, PointLightsManager, ResourcesManager,
};

pub struct Object {
    resources: ResourcesManager,

    mesh_instances: Vec<(MeshInstanceHandle, MeshInstance)>,
    point_lights: Vec<(PointLightHandle, PointLight)>,

    is_static: bool,
    transform: glam::Mat4,
}

impl Object {
    pub fn new(
        resources: &ResourcesManager,
        mesh_instances: Vec<MeshInstance>,
        point_lights: Vec<PointLight>,
    ) -> Self {
        let mesh_instances = {
            let handles = resources
                .write::<MeshInstancesManager>()
                .add(&mesh_instances);
            std::iter::zip(handles, mesh_instances).collect()
        };

        let point_lights = {
            let handles = resources.write::<PointLightsManager>().add(&point_lights);
            std::iter::zip(handles, point_lights).collect()
        };

        Self {
            resources: resources.clone(),

            mesh_instances,
            point_lights,

            is_static: false,
            transform: glam::Mat4::IDENTITY,
        }
    }

    pub fn with_transform(mut self, transform: glam::Mat4) -> Self {
        self.set_transform(transform);
        self
    }

    pub fn with_animation(mut self, animation: AnimationState) -> Self {
        self.set_animation(animation);
        self
    }

    pub fn with_static(mut self, is_static: bool) -> Self {
        self.set_static(is_static);
        self
    }

    pub fn transform(&self) -> glam::Mat4 {
        self.transform
    }

    pub fn set_transform(&mut self, transform: glam::Mat4) {
        self.resources.write::<MeshInstancesManager>().replace(
            &self
                .mesh_instances
                .iter()
                .copied()
                .map(|(mesh_instance_handle, mesh_instance)| {
                    (
                        mesh_instance_handle,
                        MeshInstance {
                            transform: transform * mesh_instance.transform,
                            ..mesh_instance
                        },
                        MeshInstanceFlags::UPDATE_TRANSFORM,
                    )
                })
                .collect::<Vec<_>>(),
        );

        self.resources.write::<PointLightsManager>().replace(
            &mut self
                .point_lights
                .iter()
                .copied()
                .map(|(point_light_handle, point_light)| {
                    (
                        point_light_handle,
                        PointLight {
                            position: transform.transform_point3(point_light.position),
                            ..point_light
                        },
                    )
                })
                .collect::<Vec<_>>(),
        );

        self.transform = transform;
    }

    pub fn set_animation(&mut self, animation: AnimationState) {
        self.resources.write::<MeshInstancesManager>().replace(
            &self
                .mesh_instances
                .iter_mut()
                .map(|(mesh_instance_handle, mesh_instance)| {
                    mesh_instance.animation = animation;

                    (
                        *mesh_instance_handle,
                        *mesh_instance,
                        MeshInstanceFlags::UPDATE_ANIMATION,
                    )
                })
                .collect::<Vec<_>>(),
        );
    }

    pub fn set_static(&mut self, is_static: bool) {
        self.is_static = is_static;
    }
}

impl Drop for Object {
    fn drop(&mut self) {
        if self.is_static {
            return;
        }

        self.resources.write::<MeshInstancesManager>().remove(
            &self
                .mesh_instances
                .iter()
                .map(|(mesh_instance_handle, _)| mesh_instance_handle)
                .copied()
                .collect::<Vec<_>>(),
        );

        self.resources.write::<PointLightsManager>().remove(
            &mut self
                .point_lights
                .iter()
                .map(|(point_light_handle, _)| point_light_handle)
                .copied()
                .collect::<Vec<_>>(),
        );
    }
}
