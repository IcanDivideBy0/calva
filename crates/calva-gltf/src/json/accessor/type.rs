use serde::{Deserialize, Deserializer};

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum AccessorType {
    Scalar,
    Vec2,
    Vec3,
    Vec4,
    Mat2,
    Mat3,
    Mat4,
}

impl<'de> Deserialize<'de> for AccessorType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        match String::deserialize(deserializer)?.as_str() {
            "SCALAR" => Ok(AccessorType::Scalar),
            "VEC2" => Ok(AccessorType::Vec2),
            "VEC3" => Ok(AccessorType::Vec3),
            "VEC4" => Ok(AccessorType::Vec4),
            "MAT2" => Ok(AccessorType::Mat2),
            "MAT3" => Ok(AccessorType::Mat3),
            "MAT4" => Ok(AccessorType::Mat4),

            value => Err(serde::de::Error::invalid_value(
                serde::de::Unexpected::Str(value),
                &r#"one of ["SCALAR", "VEC2", "VEC3", "VEC4", "MAT2", "MAT3", "MAT4"]"#,
            )),
        }
    }
}

#[test]
fn accessor_type() -> serde_json::Result<()> {
    #[derive(Deserialize)]
    struct Test {
        pub type_: AccessorType,
    }

    assert_eq!(
        serde_json::from_str::<Test>(r#"{ "type": "SCALAR" }"#)?.type_,
        AccessorType::Scalar
    );

    Ok(())
}
