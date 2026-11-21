// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_media::ExportOptions;

use super::support::{self, TestResult};

#[test]
fn checked_in_media_schemas_match_the_typed_contracts() -> TestResult {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../schemas");
    for (name, schema) in [
        ("media-index.schema.json", p4kx_media::index_schema()),
        ("media-record.schema.json", p4kx_media::record_schema()),
        (
            "media-builtin-record.schema.json",
            p4kx_media::builtin_record_schema()?,
        ),
        (
            "media-reference.schema.json",
            p4kx_media::reference_schema(),
        ),
        ("media-source.schema.json", p4kx_media::source_schema()),
    ] {
        let generated = serde_json::to_string_pretty(&schema)? + "\n";
        let path = directory.join(name);
        if std::env::var_os("P4KX_BLESS").is_some_and(|value| value == "1") {
            std::fs::write(&path, &generated)?;
        }
        assert_eq!(
            std::fs::read_to_string(&path)?,
            generated,
            "{} is stale; regenerate with P4KX_BLESS=1 cargo test -p p4kx-media --test it checked_in_media_schemas",
            path.display()
        );
        jsonschema::validator_for(&schema)?;
    }
    Ok(())
}

#[test]
fn actual_index_and_jsonl_records_validate_and_other_format_versions_are_refused() -> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = support::archive(folder.path())?;
    let output = folder.path().join("media");
    let index = support::published(p4kx_media::export(
        &archive,
        &output,
        &ExportOptions::default(),
    )?)?;
    let mut json = serde_json::to_value(&index)?;
    let schema = p4kx_media::index_schema();
    let validator = jsonschema::validator_for(&schema)?;
    validator
        .validate(&json)
        .map_err(|error| error.to_string())?;
    *json.pointer_mut("/format_version").ok_or("version field")? = serde_json::json!("3");
    assert!(
        !validator.is_valid(&json),
        "catalog version cannot masquerade as media format 1.0"
    );
    let schema = p4kx_media::record_schema();
    let validator = jsonschema::validator_for(&schema)?;
    for record in support::records(&output, &index, "archive.jsonl")? {
        validator
            .validate(&serde_json::to_value(record)?)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}
