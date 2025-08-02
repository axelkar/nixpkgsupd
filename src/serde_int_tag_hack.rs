//! Inspiration: <https://github.com/serde-rs/serde/issues/745#issuecomment-1450072069>

use serde::{Deserialize, Serialize};

#[derive(Debug)]
pub struct Version<const V: u8>;

impl<const V: u8> Serialize for Version<V> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_u8(V)
    }
}

impl<'de, const V: u8> Deserialize<'de> for Version<V> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        if u8::deserialize(deserializer)? == V {
            Ok(Self)
        } else {
            Err(serde::de::Error::custom("unsupported version"))
        }
    }
}
