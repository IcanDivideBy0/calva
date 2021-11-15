use serde::{Deserialize, Deserializer};

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum AccessorComponentType {
    Byte,
    UnsignedByte,
    Short,
    UnsignedShort,
    UnsignedInt,
    Float,
}

impl<'de> Deserialize<'de> for AccessorComponentType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        match usize::deserialize(deserializer)? {
            5120 => Ok(AccessorComponentType::Byte),
            5121 => Ok(AccessorComponentType::UnsignedByte),
            5122 => Ok(AccessorComponentType::Short),
            5123 => Ok(AccessorComponentType::UnsignedShort),
            5125 => Ok(AccessorComponentType::UnsignedInt),
            5126 => Ok(AccessorComponentType::Float),

            value => Err(serde::de::Error::invalid_value(
                serde::de::Unexpected::Unsigned(value as u64),
                &"one of [5120, 5121, 5122, 5123, 5125, 5126]",
            )),
        }
    }
}

#[test]
fn accessor_component_type() -> serde_json::Result<()> {
    #[derive(Deserialize)]
    struct Test {
        pub component_type: AccessorComponentType,
    }

    assert_eq!(
        serde_json::from_str::<Test>(r#"{ "component_type": 5120 }"#)?.component_type,
        AccessorComponentType::Byte
    );

    Ok(())
}
