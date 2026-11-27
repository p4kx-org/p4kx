// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::io;
use std::ops::Range;

use p4kx_assembly::ClothingZones;
use p4kx_extraction::{
    DataForgeOrigin, EntryOrigin, FieldOrigins, FieldPath, InputScope, SourceInput, SourceLocation,
    SourceScope, SourceUse, SourceUseRole, SourceUses,
};
use serde_json::Value;

use crate::manifest_body_zones::ClothingBodyCoverage;

pub(super) type OriginalZoneWord = Option<(EntryOrigin, Range<u64>)>;

pub(super) struct SkinZoneEvidence<'a> {
    pub(super) entry: &'a EntryOrigin,
    pub(super) node: Range<u64>,
    pub(super) word: Range<u64>,
}

pub(super) struct BodyZoneOrigins {
    uses: SourceUses,
    fields: FieldOrigins,
    covering: BTreeMap<String, Vec<SourceUse>>,
}

impl BodyZoneOrigins {
    pub(super) fn new() -> Self {
        Self {
            uses: SourceUses::with_compact_use_refs(),
            fields: FieldOrigins::new(),
            covering: BTreeMap::new(),
        }
    }

    pub(super) fn body(
        &mut self,
        words: impl IntoIterator<Item = OriginalZoneWord>,
    ) -> io::Result<()> {
        for (index, observed) in words.into_iter().enumerate() {
            let source = observed
                .map(|(entry, span)| {
                    self.uses
                        .observe(SourceInput::Archive(&entry), InputScope::Payload)?
                        .at(
                            SourceLocation::Subresource {
                                namespace: "model-byte-range".into(),
                                indices: vec![span.start, span.end],
                                field: "bodyZoneNode".into(),
                            },
                            SourceUseRole::Selector,
                        )
                })
                .transpose()
                .map_err(io::Error::other)?;
            self.insert(&format!("/body/primitives/{index}/zone"), source.as_slice())?;
            self.insert(&format!("/body/primitives/{index}/model_lod"), &[])?;
        }
        Ok(())
    }

    pub(super) fn omitted(&mut self, zones: &[String]) -> io::Result<()> {
        for (index, name) in zones.iter().enumerate() {
            let used = self.covering.remove(name).unwrap_or_default();
            self.insert(&format!("/body/omitted/{index}"), &used)?;
        }
        Ok(())
    }

    pub(super) fn clothing(
        &mut self,
        index: usize,
        clothing: &ClothingZones,
        coverage: &ClothingBodyCoverage,
    ) -> io::Result<()> {
        let scope = self.clothing_scope(clothing)?;
        let base = format!("/parts/{index}/coverage");
        self.insert_record(&base, scope.as_ref(), clothing.record_index())?;
        for (name, zones) in [
            ("covers", &coverage.covers),
            ("conditional", &coverage.conditional),
        ] {
            let path = format!("{base}/{name}");
            let mut contributors = Vec::new();
            for (position, zone) in zones.iter().enumerate() {
                let used = scope
                    .iter()
                    .flat_map(|scope| {
                        clothing
                            .coverage_sources(zone)
                            .map(move |location| scope.at(location, SourceUseRole::Conversion))
                    })
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(io::Error::other)?;
                self.insert(&format!("{path}/{position}"), &used)?;
                if name == "covers" {
                    self.covering
                        .entry(zone.clone())
                        .or_default()
                        .extend(used.iter().cloned());
                }
                contributors.extend(used);
            }
            self.insert(&path, &contributors)?;
        }
        Ok(())
    }

    pub(super) fn worn_primitive(
        &mut self,
        path: &str,
        clothing: &ClothingZones,
        zone: &str,
        geometry: Option<SkinZoneEvidence<'_>>,
    ) -> io::Result<()> {
        let mut used = Vec::new();
        if let Some(geometry) = geometry {
            let source = self
                .uses
                .observe(SourceInput::Archive(geometry.entry), InputScope::Payload)
                .map_err(io::Error::other)?;
            for (field, span) in [
                ("clothingZoneNode", geometry.node),
                ("clothingZoneHash", geometry.word),
            ] {
                let location = SourceLocation::Subresource {
                    namespace: "model-byte-range".into(),
                    indices: vec![span.start, span.end],
                    field: field.into(),
                };
                used.push(
                    source
                        .at(location, SourceUseRole::Selector)
                        .map_err(io::Error::other)?,
                );
            }
        }
        if let Some(scope) = self.clothing_scope(clothing)? {
            for location in clothing.coverage_sources(zone) {
                used.push(
                    scope
                        .at(location, SourceUseRole::Conversion)
                        .map_err(io::Error::other)?,
                );
            }
        }
        self.insert(path, &used)
    }

    fn clothing_scope(&self, clothing: &ClothingZones) -> io::Result<Option<SourceScope>> {
        match clothing.forge_origin() {
            Some(DataForgeOrigin::Archive(entry)) => Some(SourceInput::Archive(entry)),
            Some(DataForgeOrigin::File(file)) => Some(SourceInput::File(file)),
            _ => None,
        }
        .map(|source| self.uses.observe(source, InputScope::Payload))
        .transpose()
        .map_err(io::Error::other)
    }

    fn insert_record(
        &mut self,
        base: &str,
        scope: Option<&SourceScope>,
        record_index: Option<u32>,
    ) -> io::Result<()> {
        let guid = scope
            .zip(record_index)
            .map(|(scope, record_index)| {
                scope.at(
                    SourceLocation::DataForge {
                        record_index,
                        field: "$guid".into(),
                    },
                    SourceUseRole::Payload,
                )
            })
            .transpose()
            .map_err(io::Error::other)?;
        self.insert(&format!("{base}/guid"), guid.as_slice())?;
        self.insert(&format!("{base}/record_index"), &[])?;
        self.insert(&format!("{base}/record_path"), &[])
    }

    fn insert(&mut self, path: &str, uses: &[SourceUse]) -> io::Result<()> {
        self.fields
            .insert(
                FieldPath::parse(path).map_err(io::Error::other)?,
                self.uses.origin(uses, false),
            )
            .map_err(io::Error::other)
    }

    pub(super) fn finish(self) -> io::Result<(Value, Value)> {
        let inventory = self.uses.freeze().map_err(io::Error::other)?;
        Ok((
            serde_json::to_value(inventory).map_err(io::Error::other)?,
            serde_json::to_value(self.fields).map_err(io::Error::other)?,
        ))
    }
}
