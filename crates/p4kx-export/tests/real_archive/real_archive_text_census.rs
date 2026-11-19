// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::fmt;
use std::io::Read;
use std::path::Path;

use p4kx_export::{CatalogOptions, CatalogSummary, Compression, Kind, SCHEMA_VERSION};
use p4kx_extraction::Archive;
use serde::Deserialize;
use serde::de::{DeserializeSeed, IgnoredAny, MapAccess, Visitor};
use serde_json::{Map, Value};

use crate::catalog_v3_consumer;
use crate::real_archive_catalog_size::artifact;

#[path = "real_archive_item_text_capture.rs"]
mod item_capture;
#[path = "real_archive_text_pipe.rs"]
mod pipe;

#[cfg(test)]
#[path = "real_archive_text_census_tests.rs"]
mod tests;

pub(super) type TestResult<T> = Result<T, Box<dyn std::error::Error>>;

pub(super) fn write_catalog(
    archive: &Archive,
    options: &CatalogOptions<'_>,
    before_write: impl FnOnce(),
) -> TestResult<(CatalogSummary, Value)> {
    let paths = TextPaths::catalog();
    let (summary, capture) = pipe::transfer(&paths, |sink| {
        p4kx_export::write_catalog(archive, options, Compression::None, || {
            before_write();
            Ok(sink)
        })
    })?;
    if capture.bytes != summary.size() {
        return Err("text census byte count differs from the catalog writer".into());
    }
    Ok((summary, schema_v3(capture.document)?))
}

pub(super) fn read_completed_catalog(directory: &Path) -> TestResult<Value> {
    let paths = TextPaths::catalog();
    let projection = artifact::read_completed(directory, |decoded| Ok(paths.read(decoded)?))?;
    schema_v3(projection)
}

fn schema_v3(document: Value) -> TestResult<Value> {
    if document.get("schemaVersion").and_then(Value::as_u64) != Some(u64::from(SCHEMA_VERSION)) {
        return Err("the text census requires catalog schema v3".into());
    }
    Ok(document)
}

#[derive(Default)]
struct TextPaths {
    children: BTreeMap<String, Self>,
    projection: Option<item_capture::Projection>,
}

impl TextPaths {
    fn catalog() -> Self {
        let mut paths = Self::default();
        for kind in Kind::ALL {
            paths.keep(&catalog_v3_consumer::row_location(kind));
        }
        for path in [
            "/schemaVersion",
            "/source",
            "/completeness",
            "/exportHealth/status",
            "/exportHealth/read",
            "/exportHealth/projection",
            "/exportHealth/publicationMode",
            "/exportHealth/kinds",
            "/texts",
            "/configurations/contractTemplates",
            "/configurations/missionBrokerEntries",
            "/configurations/blueprintTiers",
            "/projections/blueprintPresentation",
            "/projections/commodityPresentation",
        ] {
            paths.keep(path);
        }
        paths.keep("/definitions/items").projection = Some(item_capture::ITEM_DEFINITIONS);
        paths.keep("/provenance/sources").projection =
            Some(item_capture::Projection::ArchiveEntrySources);
        paths
    }

    fn keep(&mut self, path: &str) -> &mut Self {
        let mut location = self;
        for field in path.trim_start_matches('/').split('/') {
            location = location.children.entry(field.to_owned()).or_default();
        }
        location
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
        if let Some(projection) = self.projection {
            projection.deserialize(deserializer)
        } else if self.children.is_empty() {
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
            if let Some(path) = self.children.get(&field) {
                kept.insert(field, map.next_value_seed(path)?);
            } else {
                map.next_value::<IgnoredAny>()?;
            }
        }
        Ok(Value::Object(kept))
    }
}

pub(in crate::real_archive_texts) fn read_fixture_through_pipe(input: &[u8]) -> TestResult<Value> {
    use std::io::Write as _;

    let ((), capture) = pipe::transfer(&TextPaths::catalog(), |mut sink| {
        sink.write_all(input)
            .map_err(p4kx_export::CatalogError::Write)?;
        Ok(((), sink))
    })?;
    Ok(capture.document)
}
