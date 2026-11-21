// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::model::{Material, MaterialError, MaterialLibrary, PaletteSlot};
use p4kx_extraction::{MaterialAuthoredInfo, MaterialLibrarySourceInfo};
use serde_json::{Value, json};

use super::{
    Algorithm, Artifact, ByteScope, Float32, KindSummary, MediaKind, MediaRecord, Source, Status,
    Tally, TestResult, require,
};

#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct Counts {
    pub(super) enumerated: u64,
    pub(super) selected: u64,
    pub(super) nested: u64,
    pub(super) complete: u64,
    pub(super) partial: u64,
    pub(super) failed: u64,
    pub(super) materials: usize,
    pub(super) records: u64,
}

impl Counts {
    pub(super) fn account(
        &mut self,
        row: &MediaRecord,
        original: Result<&MaterialLibrary, &MaterialError>,
    ) -> TestResult {
        match original {
            Ok(source) => {
                let complete = library(source, row)?;
                self.materials += source.materials.len();
                self.complete += u64::from(complete);
                self.partial += u64::from(!complete);
            }
            Err(error) => {
                failure(error, row)?;
                self.failed += 1;
            }
        }
        Ok(())
    }

    pub(super) fn check_totals(&self, entries: &Tally, kind: &KindSummary) -> TestResult {
        let selected_sources = self.selected;
        require(
            kind.provider.kind == MediaKind::Material,
            "actual material provider",
        )?;
        require(
            entries.enumerated == self.enumerated
                && entries.written == selected_sources
                && entries.skipped == self.enumerated - selected_sources
                && entries.failed == 0
                && entries.partial == 0,
            "material Entry counts differ",
        )?;
        let tally = &kind.counts.source_entries;
        require(
            tally.enumerated == self.selected
                && tally.written == self.complete
                && tally.partial == self.partial
                && tally.failed == self.failed
                && tally.skipped == 0,
            "material provider callback counts differ",
        )?;
        let records = &kind.counts.records;
        require(
            records.enumerated == self.records
                && records.written == self.records - self.partial - self.failed
                && records.partial == self.partial
                && records.failed == self.failed
                && records.skipped == 0,
            "material record counts differ",
        )?;
        require(
            kind.counts.converted_outputs.enumerated == 0,
            "material metadata creates no converted output",
        )
    }
}

pub(super) fn check_source(row: &Source, expected: &Source) -> TestResult {
    require(
        row.identity == expected.identity
            && row.raw == expected.raw
            && row.root == expected.root
            && row.size == expected.size
            && row.declared_integrity == expected.declared_integrity
            && row.verification == expected.verification,
        "provider and manifest agree on the verified original input",
    )
}

pub(super) fn check_artifact(artifact: &Artifact, source: &Source, size: u64) -> TestResult {
    let raw = source
        .raw
        .as_ref()
        .ok_or("original raw material artifact")?;
    require(
        raw.path == format!("raw/{}", raw.digest.hex)
            && raw.digest.algorithm == Algorithm::Sha256
            && raw.digest.scope == ByteScope::DecodedEntryPayload
            && raw.size == size,
        "raw path and decoded integrity scope agree with the original Entry",
    )?;
    require(
        artifact.path == raw.path && artifact.size == raw.size && artifact.digest == raw.digest,
        "exact raw artifact receipt",
    )
}

fn library(source: &MaterialLibrary, row: &MediaRecord) -> TestResult<bool> {
    let payload = row.data.get("payload").ok_or("actual material payload")?;
    require(
        row.subresource.namespace == "material-library",
        "material library namespace",
    )?;
    require(
        payload.get("path") == Some(&json!(source.path))
            && payload.get("single") == Some(&json!(source.single))
            && payload.get("default_palette") == Some(&json!(source.default_palette))
            && payload.get("unknown_entries") == Some(&json!(source.unknown_entries)),
        "actual root properties differ from the material reader",
    )?;
    let root = MaterialLibrarySourceInfo::of(source);
    require(
        payload.get("authored") == Some(&serde_json::to_value(&root)?),
        "actual authored root changed",
    )?;
    let materials = payload
        .get("materials")
        .and_then(Value::as_array)
        .ok_or("actual material positions")?;
    require(
        materials.len() == source.materials.len(),
        "every original slot must have a material row",
    )?;
    let mut complete = root.is_complete();
    for (index, (material, wire)) in source.materials.iter().zip(materials).enumerate() {
        complete &= authored(material, wire, index)?;
        surface(
            material,
            wire.get("interpreted_surface")
                .ok_or("actual material surface")?,
        )?;
    }
    references(source, payload)?;
    let expected = if complete {
        Status::Complete
    } else {
        Status::Partial
    };
    require(
        row.status == expected,
        "actual material projection disposition",
    )?;
    require(
        payload.get("projection_complete") == Some(&json!(complete)),
        "source completeness flag",
    )?;
    Ok(complete)
}

fn references(source: &MaterialLibrary, payload: &Value) -> TestResult {
    let mut rows = payload
        .get("references")
        .and_then(Value::as_array)
        .ok_or("actual dependency declarations")?
        .iter();
    if let Some(palette) = &source.default_palette {
        reference(rows.next(), "default-palette", None, None, None, palette)?;
    }
    for (material_index, material) in source.materials.iter().enumerate() {
        for (slot, layer) in material.layers.iter().enumerate() {
            if let Some(path) = layer.attribute("Path") {
                reference(
                    rows.next(),
                    "layer-library",
                    Some(material_index),
                    Some(slot),
                    layer.attribute("Submtl"),
                    path,
                )?;
            }
        }
        for (slot, texture) in material.textures.iter().enumerate() {
            let role = if texture.file.trim_start().starts_with('$') {
                "engine-placeholder"
            } else {
                "texture"
            };
            reference(
                rows.next(),
                role,
                Some(material_index),
                Some(slot),
                None,
                &texture.file,
            )?;
        }
    }
    require(rows.next().is_none(), "additional dependency declarations")
}

fn reference(
    row: Option<&Value>,
    role: &str,
    material: Option<usize>,
    slot: Option<usize>,
    submaterial: Option<&str>,
    token: &str,
) -> TestResult {
    let row = row.ok_or("missing dependency declaration")?;
    require(
        row.get("role").and_then(Value::as_str) == Some(role)
            && row.get("material") == Some(&json!(material))
            && row.get("slot") == Some(&json!(slot))
            && row.get("submaterial") == Some(&json!(submaterial))
            && row
                .pointer("/reference/original/value")
                .and_then(Value::as_str)
                == Some(token),
        "actual dependency role, position or original token changed",
    )
}

fn authored(source: &Material, wire: &Value, index: usize) -> TestResult<bool> {
    let expected = MaterialAuthoredInfo::of(source);
    require(
        wire.get("index") == Some(&json!(index))
            && wire.get("name") == Some(&json!(source.name))
            && wire.get("authored") == Some(&serde_json::to_value(&expected)?),
        "actual material position, name or ordered raw fields changed",
    )?;
    Ok(expected.is_complete())
}

fn surface(source: &Material, wire: &Value) -> TestResult {
    for (field, expected) in [
        ("basis", json!("existing-reader-with-defaults")),
        ("flags", json!(source.flags.0)),
        ("shader", json!(source.shader)),
        ("surface_type", json!(source.surface_type)),
        ("string_gen_mask", json!(source.string_gen_mask)),
        ("diffuse", json!(source.diffuse.map(Float32::new))),
        ("specular", json!(source.specular.map(Float32::new))),
        ("emissive", json!(source.emissive.map(Float32::new))),
        ("shininess", json!(Float32::new(source.shininess))),
        ("opacity", json!(Float32::new(source.opacity))),
        ("alpha_test", json!(Float32::new(source.alpha_test))),
        ("glow", json!(Float32::new(source.glow))),
        (
            "palette",
            json!(source.palette.map(|slot| match slot {
                PaletteSlot::Primary => "primary",
                PaletteSlot::Secondary => "secondary",
                PaletteSlot::Tertiary => "tertiary",
                PaletteSlot::Glass => "glass",
                PaletteSlot::Decal => "decal",
                _ => "unsupported",
            })),
        ),
        ("tinted", json!(source.tinted)),
        (
            "decal_colours",
            json!(
                source
                    .decal_colours
                    .map(|colours| colours.map(|colour| colour.map(Float32::new)))
            ),
        ),
        ("diffuse_map_is_grey", json!(source.diffuse_map_is_grey)),
    ] {
        require(
            wire.get(field)
                .is_some_and(|found| equal_json(found, &expected)),
            "actual interpreted material fields changed",
        )?;
    }
    Ok(())
}

fn equal_json(actual: &Value, expected: &Value) -> bool {
    match (actual, expected) {
        (Value::Array(actual), Value::Array(expected)) => {
            actual.len() == expected.len()
                && actual
                    .iter()
                    .zip(expected)
                    .all(|(actual, expected)| equal_json(actual, expected))
        }
        (Value::Object(actual), Value::Object(expected)) => {
            actual.len() == expected.len()
                && expected.iter().all(|(key, value)| {
                    actual
                        .get(key)
                        .is_some_and(|found| equal_json(found, value))
                })
        }
        (Value::Number(actual), Value::Number(expected)) if expected.is_f64() => {
            #[expect(
                clippy::cast_possible_truncation,
                reason = "these values were projected from exact source f32 words"
            )]
            let f32_bits =
                |number: &serde_json::Number| number.as_f64().map(|value| (value as f32).to_bits());
            f32_bits(actual) == f32_bits(expected)
        }
        _ => actual == expected,
    }
}

fn failure(error: &MaterialError, row: &MediaRecord) -> TestResult {
    require(
        row.status == Status::Failed && row.subresource.namespace == "material-library-failure",
        "invalid original bytes require a failed material row",
    )?;
    let expected = match error {
        MaterialError::TextEncoding(_) => "text-encoding",
        MaterialError::Xml(_) => "xml",
        MaterialError::CryXml(_) => "cry-xml",
        MaterialError::TooDeep(_) => "xml-depth",
        MaterialError::NotMaterial { .. } => "not-material",
        MaterialError::SourceBytes { .. } => "source-bytes",
        MaterialError::TooMany { .. } => "typed-limit",
        MaterialError::SourceSpan => "source-span",
        MaterialError::InvalidSourceLimits => "invalid-limits",
        _ => "unsupported-reader",
    };
    require(
        row.data
            .pointer("/payload/reason/type")
            .and_then(Value::as_str)
            == Some(expected),
        "actual structured source failure reason differs",
    )
}
