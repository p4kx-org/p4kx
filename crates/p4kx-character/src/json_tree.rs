// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt;

use serde::de::{MapAccess, SeqAccess, Visitor};
use serde::ser::{SerializeMap, SerializeSeq};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::ChfJsonName;

/// A JSON document that keeps the order of the keys, which `serde_json::Value` loses unless the
/// whole workspace turns on `preserve_order`.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Node {
    Null,
    Bool(bool),
    Unsigned(u64),
    Signed(i64),
    Float(f64),
    Text(String),
    ChfHash(ChfJsonName),
    ChfLabel(String, ChfJsonName),
    List(Vec<Self>),
    Object(Vec<(String, Self)>),
}

impl Node {
    pub(crate) fn field_mut(&mut self, key: &str) -> Option<&mut Self> {
        match self {
            Self::Object(fields) => fields
                .iter_mut()
                .find(|(name, _)| name == key)
                .map(|(_, value)| value),
            _ => None,
        }
    }

    pub(crate) fn items_mut(&mut self) -> impl Iterator<Item = &mut Self> {
        match self {
            Self::List(items) => items.as_mut_slice(),
            _ => &mut [],
        }
        .iter_mut()
    }
}

struct NodeVisitor;

impl<'de> Visitor<'de> for NodeVisitor {
    type Value = Node;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("any JSON value")
    }

    fn visit_unit<E>(self) -> Result<Node, E> {
        Ok(Node::Null)
    }

    fn visit_bool<E>(self, value: bool) -> Result<Node, E> {
        Ok(Node::Bool(value))
    }

    fn visit_u64<E>(self, value: u64) -> Result<Node, E> {
        Ok(Node::Unsigned(value))
    }

    fn visit_i64<E>(self, value: i64) -> Result<Node, E> {
        Ok(Node::Signed(value))
    }

    fn visit_f64<E>(self, value: f64) -> Result<Node, E> {
        Ok(Node::Float(value))
    }

    fn visit_str<E>(self, value: &str) -> Result<Node, E> {
        Ok(Node::Text(value.to_owned()))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Node, A::Error> {
        let mut items = Vec::with_capacity(seq.size_hint().unwrap_or(0).min(1024));
        while let Some(item) = seq.next_element()? {
            items.push(item);
        }
        Ok(Node::List(items))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Node, A::Error> {
        let mut fields = Vec::with_capacity(map.size_hint().unwrap_or(0).min(1024));
        while let Some(entry) = map.next_entry()? {
            fields.push(entry);
        }
        Ok(Node::Object(fields))
    }
}

impl<'de> Deserialize<'de> for Node {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(NodeVisitor)
    }
}

impl Serialize for Node {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Null => serializer.serialize_unit(),
            Self::Bool(value) => serializer.serialize_bool(*value),
            Self::Unsigned(value) => serializer.serialize_u64(*value),
            Self::Signed(value) => serializer.serialize_i64(*value),
            Self::Float(value) => serializer.serialize_f64(*value),
            Self::Text(value) | Self::ChfLabel(value, _) => serializer.serialize_str(value),
            Self::ChfHash(name) => serializer.serialize_u64(u64::from(name.hash())),
            Self::List(items) => {
                let mut seq = serializer.serialize_seq(Some(items.len()))?;
                for item in items {
                    seq.serialize_element(item)?;
                }
                seq.end()
            }
            Self::Object(fields) => {
                let mut map = serializer.serialize_map(Some(fields.len()))?;
                for (key, value) in fields {
                    map.serialize_entry(key, value)?;
                }
                map.end()
            }
        }
    }
}
