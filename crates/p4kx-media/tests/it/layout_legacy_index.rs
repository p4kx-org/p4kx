// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;

use p4kx_extraction::ConflictPolicy;
use p4kx_media::{ExportOptions, ExportOutcome, Index};
use sha2::{Digest as _, Sha256};

use super::support::{self, TestResult};

const LEGACY_JSON: &str = include_str!("fixtures/media-index-v1-no-metadata-work.json");
const LEGACY_GENERATION: &str = "637d631a598dafd39547a5de3c979de062a6dc0e5d50e6b9a1fdc261243ce350";

#[test]
fn independently_authored_legacy_index_keeps_its_known_generation_hash() -> TestResult {
    let mut index: Index = serde_json::from_str(LEGACY_JSON)?;
    assert_eq!(index.generation, LEGACY_GENERATION);
    assert_eq!(index.limits.metadata_work, 100_000_000);
    index.generation.clear();
    index.artifact_base.clear();
    let bytes = serde_json::to_vec(&index)?;
    let digest: [u8; 32] = Sha256::digest(&bytes).into();
    assert_eq!(
        p4kx_extraction::Checksum::Sha256(digest).hex(),
        LEGACY_GENERATION,
        "canonical default omission preserves the independent pre-extension digest"
    );
    let mut limits = serde_json::to_value(index.limits)?;
    assert!(
        limits.get("metadata_work").is_none(),
        "the canonical default is omitted"
    );
    index.limits.metadata_work = 17;
    limits = serde_json::to_value(index.limits)?;
    assert_eq!(
        limits.get("metadata_work"),
        Some(&serde_json::json!(17)),
        "nondefault work bounds remain explicit"
    );
    Ok(())
}

#[test]
fn a_legacy_generation_is_validated_then_rebuilt_with_current_field_origins() -> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = support::archive(folder.path())?;
    let output = folder.path().join("media");
    let generation = output.join("generations").join(LEGACY_GENERATION);
    fs::create_dir_all(&generation)?;
    fs::write(generation.join("sources.jsonl"), b"")?;
    fs::write(generation.join("artifacts.jsonl"), b"")?;
    fs::write(output.join("media-index.json"), LEGACY_JSON)?;
    let current = support::published(p4kx_media::export(
        &archive,
        &output,
        &ExportOptions::default(),
    )?)?;
    assert_ne!(current.generation, LEGACY_GENERATION);
    assert!(
        current.field_origins.is_some(),
        "current provenance is regenerated before reuse"
    );
    let committed = fs::read(output.join("media-index.json"))?;
    let ExportOutcome::Skipped { index } =
        p4kx_media::export(&archive, &output, &ExportOptions::default())?
    else {
        return Err("the current validated generation was not reused".into());
    };
    assert_eq!(index.generation, current.generation);
    assert_eq!(fs::read(output.join("media-index.json"))?, committed);
    let options = ExportOptions {
        conflict: ConflictPolicy::Overwrite,
        ..ExportOptions::default()
    };
    let index = support::published(p4kx_media::export(&archive, &output, &options)?)?;
    assert_ne!(index.generation, LEGACY_GENERATION);
    assert_eq!(fs::read(generation.join("sources.jsonl"))?, b"");
    assert_eq!(fs::read(generation.join("artifacts.jsonl"))?, b"");
    let validator = jsonschema::validator_for(&p4kx_media::index_schema())?;
    assert!(
        validator.is_valid(&serde_json::from_str::<serde_json::Value>(LEGACY_JSON)?),
        "old bounds omit only the normative metadata-work default"
    );
    Ok(())
}

#[test]
fn metadata_work_schema_exposes_the_optional_normative_default() -> TestResult {
    let schema = p4kx_media::index_schema();
    let limits = schema.pointer("/$defs/Limits").ok_or("limits schema")?;
    assert_eq!(
        limits.pointer("/properties/metadata_work/default"),
        Some(&serde_json::json!(100_000_000)),
        "consumers can recover the normative omitted metadata work bound"
    );
    let required = limits
        .get("required")
        .and_then(serde_json::Value::as_array)
        .ok_or("required limits")?;
    assert!(
        !required
            .iter()
            .any(|name| name.as_str() == Some("metadata_work")),
        "the normative default remains optional for existing indexes"
    );
    Ok(())
}
