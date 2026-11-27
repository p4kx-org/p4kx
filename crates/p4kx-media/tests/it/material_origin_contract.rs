// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::io::Write as _;

use p4kx_extraction::ConflictPolicy;
use p4kx_extraction::model::MaterialSourceLimits;
use p4kx_media::{
    Capability, ExportOutcome, MaterialProvider, MediaError, MediaWriter, Projection, Provider,
    ProviderRegistry, SourceContext, Status, Subresource,
};
use serde_json::{Value, json};

use super::material_origin_support::{self as support, Fixture, MTL, Storage, TestResult};

#[test]
fn material_container_origins_union_only_their_actual_retained_children() -> TestResult {
    let source = b"<Material Shader='demo' StringGenMask='%A'/>";
    let fixture = Fixture::new([
        support::material(MTL, source, Storage::Plain),
        support::shader(Storage::Encrypted),
    ])?;
    let surface = "/data/payload/materials/0/interpreted_surface";
    fixture.marker(MTL, surface, Some(false))?;
    fixture.no_payload(MTL, surface, support::SHADER)?;
    fixture.marker(MTL, "/data/payload/shader_references/0", Some(true))?;
    fixture.marker(MTL, "/data/payload", Some(false))?;
    assert_eq!(
        fixture.field(MTL, surface)?.get("sourceSetComplete"),
        Some(&Value::Bool(true)),
        "a fully mapped material-only subtree has its complete exact use set"
    );
    Ok(())
}

#[test]
fn identical_decoded_materials_keep_distinct_plain_and_encrypted_field_origins() -> TestResult {
    let first = "Data/plain.mtl";
    let second = "Data/encrypted.mtl";
    let source = b"<Material Name='same'/>";
    let fixture = Fixture::new([
        support::material(first, source, Storage::Plain),
        support::material(second, source, Storage::Encrypted),
    ])?;
    assert_eq!(
        fixture.record(first)?.1.pointer("/source/raw/digest"),
        fixture.record(second)?.1.pointer("/source/raw/digest"),
        "actual retained decoded bytes share their digest"
    );
    let pointer = "/data/payload/materials/0/name";
    fixture.marker(first, pointer, Some(false))?;
    fixture.marker(second, pointer, Some(true))?;
    assert_ne!(
        support::references(fixture.field(first, pointer)?)?,
        support::references(fixture.field(second, pointer)?)?,
        "physical source uses are never deduplicated by equal bytes"
    );
    Ok(())
}

#[test]
fn current_material_origin_receipts_support_skip_and_reject_changed_companion_bytes() -> TestResult
{
    let fixture = Fixture::new([support::material(MTL, b"<Material/>", Storage::Plain)])?;
    assert!(
        fixture.index.complete,
        "the source fixture is genuinely complete"
    );
    let mut options = fixture.options.clone();
    options.conflict = ConflictPolicy::Skip;
    let registry = support::registry(MaterialSourceLimits::default())?;
    assert!(
        matches!(
            p4kx_media::export_with(&fixture.archive, &fixture.output, &options, &registry)?,
            ExportOutcome::Skipped { .. }
        ),
        "the actual complete Material generation is reusable"
    );
    let before = fs::read(fixture.output.join("media-index.json"))?;
    fs::OpenOptions::new()
        .append(true)
        .open(fixture.base.join("field-origins.jsonl"))?
        .write_all(b"\n")?;
    assert!(
        p4kx_media::export_with(&fixture.archive, &fixture.output, &options, &registry).is_err(),
        "changed exact companion bytes cannot restore original proof"
    );
    assert_eq!(
        fs::read(fixture.output.join("media-index.json"))?,
        before,
        "failed validation preserves the committed index"
    );
    Ok(())
}

struct ImpersonatingMaterial;

impl Provider for ImpersonatingMaterial {
    fn capability(&self) -> Capability {
        MaterialProvider::default().capability()
    }

    fn source(
        &self,
        _context: &SourceContext<'_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<Projection, MediaError> {
        writer.record(
            &Subresource::new("material-library", Vec::new()),
            Status::Complete,
            &json!({"record_type":"library","payload":{"same":"public lookalike"}}),
            &[],
        )?;
        Ok(Projection::Written)
    }
}

#[test]
fn public_material_capability_and_namespace_cannot_impersonate_the_owned_origin_profile()
-> TestResult {
    let fixture = Fixture::new([support::material(MTL, b"<Material/>", Storage::Plain)])?;
    let output = fixture.folder.path().join("custom");
    let registry = ProviderRegistry::try_new(vec![Box::new(ImpersonatingMaterial)])?;
    let mut options = support::options();
    options.kinds = vec![p4kx_media::MediaKind::Material];
    let ExportOutcome::Published { index, .. } =
        p4kx_media::export_with(&fixture.archive, &output, &options, &registry)?
    else {
        return Err("custom complete outcome".into());
    };
    let rows = support::rows(
        output
            .join(&index.artifact_base)
            .join("field-origins.jsonl"),
    )?;
    let profiles: Vec<_> = rows
        .iter()
        .filter(|row| {
            row.get("type").and_then(Value::as_str) == Some("occurrence")
                && row.pointer("/scope/stream/kind").and_then(Value::as_str) == Some("material")
        })
        .map(|row| row.get("profile").and_then(Value::as_str))
        .collect();
    assert_eq!(
        profiles,
        [Some("unmigrated")],
        "public writer.record cannot obtain sealed Material mapping authority"
    );
    Ok(())
}
