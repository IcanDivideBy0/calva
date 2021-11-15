use serde::{Deserialize, Deserializer};

use super::super::{Accessor, Document};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Sampler {
    pub input: usize,
    pub output: usize,
    pub interpolation: SamplerInterpolation,
}

impl Sampler {
    pub fn input<'a: 'b, 'b>(&'a self, doc: &'b Document) -> &'b Accessor {
        doc.accessors.get(self.input).unwrap()
    }

    pub fn output<'a: 'b, 'b>(&'a self, doc: &'b Document) -> &'b Accessor {
        doc.accessors.get(self.output).unwrap()
    }
}

#[derive(Debug)]
pub enum SamplerInterpolation {
    Linear,
    Step,
    CubicSpline,
}

impl<'de> Deserialize<'de> for SamplerInterpolation {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        match String::deserialize(deserializer)?.as_str() {
            "LINEAR" => Ok(SamplerInterpolation::Linear),
            "STEP" => Ok(SamplerInterpolation::Step),
            "CUBICSPLINE" => Ok(SamplerInterpolation::CubicSpline),

            value => Err(serde::de::Error::invalid_value(
                serde::de::Unexpected::Str(value),
                &r#"one of ["LINEAR", "STEP", "CUBICSPLINE"]"#,
            )),
        }
    }
}
