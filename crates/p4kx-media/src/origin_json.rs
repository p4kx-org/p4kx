// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt;

use p4kx_extraction::FieldPath;
use serde::de::{DeserializeSeed, Deserializer, MapAccess, SeqAccess, Visitor};

use crate::MediaError;

/// Visits every serialized JSON node in original depth-first order.
pub(crate) fn visit(
    bytes: &[u8],
    max_depth: u32,
    visit: &mut dyn FnMut(&str) -> Result<(), MediaError>,
) -> Result<(), MediaError> {
    let mut error = None;
    let mut path = FieldPath::root();
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    deserializer.disable_recursion_limit();
    let result = Node {
        path: &mut path,
        remaining: max_depth,
        visit,
        error: &mut error,
    }
    .deserialize(&mut deserializer)
    .and_then(|()| deserializer.end());
    error.map_or_else(|| result.map_err(Into::into), Err)
}

struct Node<'a> {
    path: &'a mut FieldPath,
    remaining: u32,
    visit: &'a mut dyn FnMut(&str) -> Result<(), MediaError>,
    error: &'a mut Option<MediaError>,
}

impl<'de> DeserializeSeed<'de> for Node<'_> {
    type Value = ();

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<(), D::Error> {
        use serde::de::Error as _;
        if let Err(error) = (self.visit)(self.path.as_str()) {
            *self.error = Some(error);
            return Err(D::Error::custom("field-address visitor failed"));
        }
        deserializer.deserialize_any(self)
    }
}

impl Node<'_> {
    fn child(&mut self) -> Result<Node<'_>, &'static str> {
        let remaining = self
            .remaining
            .checked_sub(1)
            .ok_or("media value_depth budget exceeded")?;
        Ok(Node {
            path: self.path,
            remaining,
            visit: self.visit,
            error: self.error,
        })
    }
}

impl<'de> Visitor<'de> for Node<'_> {
    type Value = ();

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("one admitted JSON value")
    }

    fn visit_map<M: MapAccess<'de>>(mut self, mut map: M) -> Result<(), M::Error> {
        use serde::de::Error as _;
        self.child().map_err(M::Error::custom)?;
        while let Some(field) = map.next_key::<String>()? {
            self.path.push_field(&field).map_err(M::Error::custom)?;
            map.next_value_seed(self.child().map_err(M::Error::custom)?)?;
            self.path.pop();
        }
        Ok(())
    }

    fn visit_seq<S: SeqAccess<'de>>(mut self, mut sequence: S) -> Result<(), S::Error> {
        use serde::de::Error as _;
        let mut index = 0_usize;
        loop {
            self.path.push_index(index).map_err(S::Error::custom)?;
            let element = sequence.next_element_seed(self.child().map_err(S::Error::custom)?);
            self.path.pop();
            if element?.is_none() {
                return Ok(());
            }
            index = index
                .checked_add(1)
                .ok_or_else(|| S::Error::custom("JSON array position overflow"))?;
        }
    }

    fn visit_bool<E: serde::de::Error>(self, _: bool) -> Result<(), E> {
        Ok(())
    }

    fn visit_i64<E: serde::de::Error>(self, _: i64) -> Result<(), E> {
        Ok(())
    }

    fn visit_u64<E: serde::de::Error>(self, _: u64) -> Result<(), E> {
        Ok(())
    }

    fn visit_f64<E: serde::de::Error>(self, _: f64) -> Result<(), E> {
        Ok(())
    }

    fn visit_str<E: serde::de::Error>(self, _: &str) -> Result<(), E> {
        Ok(())
    }

    fn visit_unit<E: serde::de::Error>(self) -> Result<(), E> {
        Ok(())
    }
}

#[cfg(test)]
#[path = "origin_json_tests.rs"]
mod tests;
