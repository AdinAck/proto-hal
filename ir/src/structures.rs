use std::{
    collections::HashMap,
    marker::PhantomData,
    ops::{Deref, DerefMut},
};

use serde::{
    de::{self, Visitor},
    ser::SerializeSeq,
    Deserialize, Serialize,
};

pub mod entitlement;
pub mod field;
pub mod field_array;
pub mod hal;
pub mod interrupts;
pub mod peripheral;
pub mod register;
pub mod schema;
pub mod variant;
pub mod variant_array;

pub struct Validated<S> {
    structure: S,
}

impl<S> Deref for Validated<S> {
    type Target = S;

    fn deref(&self) -> &Self::Target {
        &self.structure
    }
}

impl<S> DerefMut for Validated<S> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.structure
    }
}

pub trait Ident {
    fn ident(&self) -> &str;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Collection<T: Ident> {
    map: HashMap<String, T>,
}

impl<T: Ident> Collection<T> {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }
}

impl<T: Ident> Deref for Collection<T> {
    type Target = HashMap<String, T>;

    fn deref(&self) -> &Self::Target {
        &self.map
    }
}

impl<T: Ident> DerefMut for Collection<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.map
    }
}

impl<T: Ident + Serialize> Serialize for Collection<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let vec = self.values().collect::<Vec<_>>();
        let mut seq = serializer.serialize_seq(Some(vec.len()))?;

        for item in vec {
            seq.serialize_element(item)?;
        }

        seq.end()
    }
}

struct CollectionVisitor<T: Ident> {
    _p: PhantomData<T>,
}

impl<'de, T: Ident + Deserialize<'de>> Visitor<'de> for CollectionVisitor<T> {
    type Value = Vec<T>;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("proto-hal structure")
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: de::SeqAccess<'de>,
    {
        let mut vec = Self::Value::new();

        while let Some(item) = seq.next_element()? {
            vec.push(item);
        }

        Ok(vec)
    }
}

impl<'de, T: Ident + Deserialize<'de>> Deserialize<'de> for Collection<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let vec = deserializer.deserialize_seq(CollectionVisitor {
            _p: PhantomData::<T>,
        })?;

        let mut map = HashMap::new();

        for item in vec {
            if let Some(existing) = map.insert(item.ident().to_string(), item) {
                Err(de::Error::custom(format!(
                    "Structure with ident {} already exists.",
                    existing.ident()
                )))?
            }
        }

        Ok(Self { map })
    }
}
