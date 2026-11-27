// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::path::Path;

use p4kx_media::{Identity, Index, MediaKind, MediaRecord, reconstruct_material_library};
use p4kx_testkit::{ArchiveBuilder, TestEntry};
use serde_json::{Value, json};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[test]
fn standalone_material_cli_export_keeps_original_bytes_and_uses_the_builtin_schema() -> TestResult {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    let source = br#"<Material RootOnly="keep"><MatLayers><Layer Path="missing" Submtl="007"/></MatLayers></Material>"#;
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/unused.mtl", source))
        .write_to(&path)?;
    let output = folder.path().join("material");
    p4kx_testkit::p4kx()?
        .args(["media", "export"])
        .arg(&path)
        .arg("--out")
        .arg(&output)
        .args(["--kind", "material"])
        .assert()
        .code(0);
    let index: Index = serde_json::from_slice(&fs::read(output.join("media-index.json"))?)?;
    let [kind] = index.kinds.as_slice() else {
        return Err("only the selected standalone material provider should run".into());
    };
    assert_eq!(
        kind.provider.kind,
        MediaKind::Material,
        "explicit selection runs the material provider"
    );
    assert_eq!(
        kind.counts.source_entries.written, 1,
        "all rows belong to one unused material source"
    );
    assert_eq!(
        kind.counts.converted_outputs.enumerated, 0,
        "a standalone material requires no geometry conversion"
    );
    let artifact_base = output.join(&index.artifact_base);
    let rows = read_stream(&artifact_base.join(&kind.artifact.path))?;
    let library = reconstruct_material_library(&rows)?;
    let raw = library.source.raw.as_ref().ok_or("exact original source")?;
    assert_eq!(
        fs::read(artifact_base.join(&raw.path))?,
        source,
        "CLI retains the original decoded XML bytes independently"
    );
    validate_library(&library)
}

fn read_stream(path: &Path) -> TestResult<Vec<MediaRecord>> {
    let validator = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    let values: Vec<Value> = fs::read_to_string(path)?
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()?;
    for value in &values {
        validator
            .validate(value)
            .map_err(|error| error.to_string())?;
    }
    let [_, _, reference, _] = values.as_slice() else {
        return Err("expected header, authored parts, layer reference and end".into());
    };
    let mut wrong = reference.clone();
    *wrong
        .pointer_mut("/subresource/namespace")
        .ok_or("record namespace")? = Value::String("material-library-header".into());
    assert!(
        !validator.is_valid(&wrong),
        "the library-header namespace rejects an otherwise valid reference payload"
    );
    let rows: Vec<MediaRecord> = values
        .into_iter()
        .map(serde_json::from_value)
        .collect::<Result<_, _>>()?;
    assert_eq!(
        rows.iter()
            .map(|row| row.subresource.namespace.as_str())
            .collect::<Vec<_>>(),
        [
            "material-library-header",
            "material-library-part",
            "material-reference",
            "material-library-end",
        ],
        "the stream contains only the standalone library and its authored metadata"
    );
    Ok(rows)
}

fn validate_library(record: &MediaRecord) -> TestResult {
    let Identity::Entry { containers, entry } = &record.source.identity else {
        return Err("the standalone library comes from a physical entry".into());
    };
    assert!(containers.is_empty(), "the source is in the root archive");
    assert_eq!(
        entry.path.replace('\\', "/"),
        "Data/unused.mtl",
        "the reconstructed library retains its source identity"
    );
    assert_eq!(
        record
            .data
            .pointer("/payload/materials")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(1),
        "the selected source contributes exactly one material"
    );
    assert_eq!(
        record.data.pointer("/payload/references"),
        Some(&json!([{
            "role": "layer-library",
            "material": 0,
            "slot": 0,
            "submaterial": "007",
            "reference": {
                "original": {"type": "path", "value": "missing"},
                "resolution": {"status": "missing"},
                "observed_candidates": 0,
                "candidates_complete": true
            }
        }])),
        "the sole layer dependency retains the original path and Submtl name"
    );
    Ok(())
}
