use std::marker::PhantomData;

use base64::prelude::*;

// Wrapper for (de)serializing buffers as Base64
// Adapted from https://users.rust-lang.org/t/serialize-a-vec-u8-to-json-as-base64/57781/5

#[derive(Debug, Clone)]
pub struct Base64<'a>(pub std::borrow::Cow<'a, [u8]>);

impl<'a> Base64<'a> {
    pub const fn from_slice(slice: &'a [u8]) -> Self {
        Self(std::borrow::Cow::Borrowed(slice))
    }
}

impl<'a> From<Vec<u8>> for Base64<'a> {
    fn from(value: Vec<u8>) -> Self {
        Self(std::borrow::Cow::Owned(value))
    }
}

impl<'a> serde::Serialize for Base64<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&BASE64_STANDARD.encode(&self.0))
    }
}

impl<'de: 'a, 'a> serde::Deserialize<'de> for Base64<'a> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct Vis<'b>(PhantomData<&'b ()>);
        impl<'b> serde::de::Visitor<'_> for Vis<'b> {
            type Value = Base64<'b>;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a base64 string")
            }

            fn visit_str<E: serde::de::Error>(self, input: &str) -> Result<Self::Value, E> {
                BASE64_STANDARD
                    .decode(input)
                    .map(Base64::from)
                    .map_err(serde::de::Error::custom)
            }
        }
        deserializer.deserialize_str(Vis::<'a>(PhantomData))
    }
}
