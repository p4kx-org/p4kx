// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::fmt;
use std::io::Read;

use p4kx_export::{CatalogOptions, CatalogSummary, Compression, Kind, SCHEMA_VERSION};
use p4kx_extraction::Archive;
use serde::Deserialize;
use serde::de::{DeserializeSeed, IgnoredAny, MapAccess, Visitor};
use serde_json::{Map, Value};

use crate::catalog_v3_consumer;

#[path = "real_archive_text_pipe.rs"]
mod pipe;

#[cfg(test)]
#[path = "real_archive_text_census_tests.rs"]
mod tests;

type TestResult<T> = Result<T, Box<dyn std::error::Error>>;

pub(super) fn write_catalog(
    archive: &Archive,
    options: &CatalogOptions<'_>,
) -> TestResult<(CatalogSummary, Value)> {
    let paths = TextPaths::catalog();
    let (summary, capture) = pipe::transfer(&paths, |sink| {
        p4kx_export::write_catalog(archive, options, Compression::None, || Ok(sink))
    })?;
    if capture.bytes != summary.size() {
        return Err("text census byte count differs from the catalog writer".into());
    }
    if capture
        .document
        .get("schemaVersion")
        .and_then(Value::as_u64)
        != Some(u64::from(SCHEMA_VERSION))
    {
        return Err("the text census requires catalog schema v3".into());
    }
    Ok((summary, capture.document))
}

#[derive(Default)]
struct TextPaths(BTreeMap<String, Self>);

impl TextPaths {
    fn catalog() -> Self {
        let mut paths = Self::default();
        for kind in Kind::ALL {
            paths.keep(&catalog_v3_consumer::row_location(kind));
        }
        for path in [
            "/schemaVersion",
            "/source",
            "/texts",
            "/definitions/items",
            "/configurations/contractTemplates",
            "/configurations/missionBrokerEntries",
            "/configurations/blueprintTiers",
            "/projections/blueprintPresentation",
            "/projections/commodityPresentation",
        ] {
            paths.keep(path);
        }
        paths
    }

    fn keep(&mut self, path: &str) {
        let mut location = self;
        for field in path.trim_start_matches('/').split('/') {
            location = location.0.entry(field.to_owned()).or_default();
        }
    }

    fn read(&self, input: impl Read) -> serde_json::Result<Value> {
        let mut decoder = serde_json::Deserializer::from_reader(input);
        let projected = self.deserialize(&mut decoder)?;
        decoder.end()?;
        Ok(projected)
    }
}

impl<'de> DeserializeSeed<'de> for &TextPaths {
    type Value = Value;

    fn deserialize<D: serde::Deserializer<'de>>(self, deserializer: D) -> Result<Value, D::Error> {
        if self.0.is_empty() {
            Value::deserialize(deserializer)
        } else {
            deserializer.deserialize_map(self)
        }
    }
}

impl<'de> Visitor<'de> for &TextPaths {
    type Value = Value;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a catalog section object")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Value, A::Error> {
        let mut kept = Map::new();
        while let Some(field) = map.next_key::<String>()? {
            if let Some(path) = self.0.get(&field) {
                kept.insert(field, map.next_value_seed(path)?);
            } else {
                map.next_value::<IgnoredAny>()?;
            }
        }
        Ok(Value::Object(kept))
    }
}
