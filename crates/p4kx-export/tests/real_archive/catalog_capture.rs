// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::fmt;
use std::io::Read;

use serde::Deserialize;
use serde::de::{DeserializeSeed, IgnoredAny, MapAccess, Visitor};
use serde_json::{Map, Value};

pub(crate) struct CatalogCapture<P> {
    children: BTreeMap<String, Self>,
    projection: Option<P>,
}

impl<P> Default for CatalogCapture<P> {
    fn default() -> Self {
        Self {
            children: BTreeMap::new(),
            projection: None,
        }
    }
}

impl<P> CatalogCapture<P> {
    pub(crate) fn keep(&mut self, path: &str) -> &mut Self {
        let mut location = self;
        for field in path.trim_start_matches('/').split('/') {
            location = location.children.entry(field.to_owned()).or_default();
        }
        location
    }

    pub(crate) fn project(&mut self, path: &str, projection: P) {
        self.keep(path).projection = Some(projection);
    }

    pub(crate) fn read(&self, input: impl Read) -> serde_json::Result<Value>
    where
        P: for<'de> DeserializeSeed<'de, Value = Value> + Copy,
    {
        let mut decoder = serde_json::Deserializer::from_reader(input);
        let projected = self.deserialize(&mut decoder)?;
        decoder.end()?;
        Ok(projected)
    }
}

impl<'de, P: DeserializeSeed<'de, Value = Value> + Copy> DeserializeSeed<'de>
    for &CatalogCapture<P>
{
    type Value = Value;

    fn deserialize<D: serde::Deserializer<'de>>(self, deserializer: D) -> Result<Value, D::Error> {
        if let Some(projection) = self.projection {
            projection.deserialize(deserializer)
        } else if self.children.is_empty() {
            Value::deserialize(deserializer)
        } else {
            deserializer.deserialize_map(self)
        }
    }
}

impl<'de, P: DeserializeSeed<'de, Value = Value> + Copy> Visitor<'de> for &CatalogCapture<P> {
    type Value = Value;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a catalog section object")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Value, A::Error> {
        let mut kept = Map::new();
        while let Some(field) = map.next_key::<String>()? {
            if let Some(path) = self.children.get(&field) {
                let value = map.next_value_seed(path)?;
                if kept.insert(field, value).is_some() {
                    return Err(serde::de::Error::custom("duplicate catalog section"));
                }
            } else {
                map.next_value::<IgnoredAny>()?;
            }
        }
        Ok(Value::Object(kept))
    }
}
