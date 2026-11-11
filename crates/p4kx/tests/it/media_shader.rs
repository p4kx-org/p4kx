// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::path::{Path, PathBuf};

use p4kx_media::{Index, MediaKind, MediaRecord};
use p4kx_testkit::{ArchiveBuilder, TestContainer, TestEntry};
use serde_json::Value;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn source(directory: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let nested = TestContainer::new()
        .entry(TestEntry::stored(
            "Shaders/Synthetic.EXT",
            b"Version(1)\nProperty { Name=%SYNTHETIC; Mask(0x20000000000001); Hidden; }",
        ))
        .entry(TestEntry::stored("Shaders/include.CFIB", b"opaque include"))
        .entry(TestEntry::stored("Shaders/effect.cfxb", b"opaque effect"))
        .entry(TestEntry::stored("Shaders/cache.shcb", b"opaque shader"));
    let source = directory.join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("synthetic.pak", nested.build()))
        .write_to(&source)?;
    Ok(source)
}

fn records(output: &Path, index: &Index) -> Result<Vec<MediaRecord>, Box<dyn std::error::Error>> {
    fs::read_to_string(output.join(&index.artifact_base).join("shader.jsonl"))?
        .lines()
        .map(|line| serde_json::from_str(line).map_err(Into::into))
        .collect()
}

fn record_schema() -> Result<Value, Box<dyn std::error::Error>> {
    let output = p4kx_testkit::p4kx()?
        .args(["media", "schema", "--record"])
        .assert()
        .code(0)
        .get_output()
        .stdout
        .clone();
    Ok(serde_json::from_slice(&output)?)
}

#[test]
fn shader_cli_exports_nested_extension_and_all_three_opaque_cache_families() -> TestResult {
    let folder = tempfile::tempdir()?;
    let source = source(folder.path())?;
    let output = folder.path().join("export");
    p4kx_testkit::p4kx()?
        .args(["media", "export"])
        .arg(&source)
        .arg("--out")
        .arg(&output)
        .args(["--kind", "shader", "--json"])
        .assert()
        .code(0);
    let index: Index = serde_json::from_slice(&fs::read(output.join("media-index.json"))?)?;
    assert_eq!(
        index.kinds.len(),
        1,
        "explicit shader selection does not emit the Archive provider stream"
    );
    assert_eq!(
        index.kinds.first().ok_or("shader summary")?.provider.kind,
        MediaKind::Shader,
        "the built-in shader provider is available"
    );
    let rows = records(&output, &index)?;
    assert_eq!(
        rows.len(),
        9,
        "one definition header plus five declarations and three cache inventories"
    );
    let schema = record_schema()?;
    let validator = jsonschema::validator_for(&schema)?;
    for row in &rows {
        validator
            .validate(&serde_json::to_value(row)?)
            .map_err(|error| error.to_string())?;
    }
    verify_payloads(&rows);
    Ok(())
}

fn verify_payloads(rows: &[MediaRecord]) {
    let definitions = rows
        .iter()
        .filter(|row| row.subresource.namespace == "shader-definition")
        .count();
    let properties = rows
        .iter()
        .filter(|row| row.subresource.namespace == "shader-property")
        .count();
    assert_eq!(
        (definitions, properties),
        (1, 1),
        "source definition and Property counting units remain separate"
    );
    let caches: Vec<_> = rows
        .iter()
        .filter_map(|row| row.data.pointer("/payload/family").and_then(Value::as_str))
        .collect();
    assert_eq!(
        caches,
        ["cfib", "cfxb", "shcb"],
        "compiled caches are inventoried in source order"
    );
    let word = rows.iter().find_map(|row| {
        row.data
            .pointer("/payload/meaning/interpretation/word")
            .and_then(Value::as_str)
    });
    assert_eq!(
        word,
        Some("9007199254740993"),
        "mask bits above JSON's safe integer range remain exact decimal text"
    );
    for row in rows {
        assert!(
            row.source.raw.is_some(),
            "every emitted shader unit has usable retained source bytes"
        );
        if row.subresource.namespace == "shader-cache-inventory" {
            assert_eq!(
                row.data
                    .pointer("/payload/semantic_decode")
                    .and_then(Value::as_str),
                Some("unsupported"),
                "opaque cache bytes never acquire a semantic decoder claim"
            );
        }
    }
}

#[test]
fn omission_selects_all_registered_providers() -> TestResult {
    let folder = tempfile::tempdir()?;
    let source = source(folder.path())?;
    let output = folder.path().join("export");
    p4kx_testkit::p4kx()?
        .args(["media", "export"])
        .arg(&source)
        .arg("--out")
        .arg(&output)
        .assert()
        .code(0);
    let index: Index = serde_json::from_slice(&fs::read(output.join("media-index.json"))?)?;
    let kinds: Vec<_> = index.kinds.iter().map(|kind| kind.provider.kind).collect();
    assert_eq!(
        kinds,
        [
            MediaKind::Archive,
            MediaKind::Shader,
            MediaKind::Material,
            MediaKind::Audio
        ],
        "omission preserves registration order and includes the new available provider"
    );
    assert!(
        output
            .join(&index.artifact_base)
            .join("archive.jsonl")
            .is_file(),
        "permanent Archive metadata remains available"
    );
    assert!(
        output
            .join(&index.artifact_base)
            .join("shader.jsonl")
            .is_file(),
        "shader stream remains available in the complete default selection"
    );
    let material = output.join(&index.artifact_base).join("material.jsonl");
    assert_eq!(
        fs::read(material)?,
        b"",
        "default material provider emits no invented row for a shader-only fixture"
    );
    let audio = output.join(&index.artifact_base).join("audio.jsonl");
    assert_eq!(
        fs::read(audio)?,
        b"",
        "default Audio emits no invented row for a shader-only fixture"
    );

    Ok(())
}

#[test]
fn cli_record_schema_uses_precise_builtins_while_generic_library_schema_stays_compatible()
-> TestResult {
    let schema = record_schema()?;
    assert_eq!(
        schema,
        p4kx_media::builtin_record_schema()?,
        "CLI schema follows the fallible common built-in factory"
    );
    let strict = jsonschema::validator_for(&schema)?;
    let generic = jsonschema::validator_for(&p4kx_media::record_schema())?;
    let folder = tempfile::tempdir()?;
    let source = source(folder.path())?;
    let output = folder.path().join("export");
    p4kx_testkit::p4kx()?
        .args(["media", "export"])
        .arg(&source)
        .arg("--out")
        .arg(&output)
        .args(["--kind", "shader"])
        .assert()
        .code(0);
    let index: Index = serde_json::from_slice(&fs::read(output.join("media-index.json"))?)?;
    let rows = records(&output, &index)?;
    let first = rows.first().ok_or("definition fixture")?;
    let mut mutation = serde_json::to_value(first)?;
    *mutation
        .pointer_mut("/data/payload/counts/definitions")
        .ok_or("definition count")? = serde_json::json!(0);
    assert!(
        !strict.is_valid(&mutation),
        "CLI schema refuses invalid built-in payloads"
    );
    assert!(
        generic.is_valid(&mutation),
        "existing generic library envelope does not acquire new data restrictions"
    );
    *mutation
        .pointer_mut("/subresource/namespace")
        .ok_or("provider namespace")? = serde_json::json!("custom.shader");
    strict
        .validate(&mutation)
        .map_err(|error| error.to_string())?;
    Ok(())
}
