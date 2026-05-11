use std::time::{Duration, Instant};

use anyhow::Result;

use crate::{Resource, ResourcesManager, UniformData};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Time {
    pub time: Instant,
    pub dt: Duration,

    epoch: Instant,
}

impl UniformData for Time {
    type GpuType = [f32; 2];

    fn as_gpu_type(&self) -> Self::GpuType {
        [
            self.time.duration_since(self.epoch).as_secs_f32(),
            self.dt.as_secs_f32(),
        ]
    }
}

impl Resource for Time {
    fn instanciate(_resources: &super::ResourcesManager) -> Self {
        Self {
            time: Instant::now(),
            dt: Duration::default(),

            epoch: Instant::now(),
        }
    }

    fn update(&mut self, _resources: &ResourcesManager) -> Result<()> {
        self.dt = self.time.elapsed();
        self.time = Instant::now();

        Ok(())
    }
}
