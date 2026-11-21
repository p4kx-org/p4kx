// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::path::Path;

use p4kx_extraction::Filter;
use p4kx_extraction::chunks::ivo;
use p4kx_media::{ExportOptions, MediaKind, MediaRecord};
use p4kx_testkit::{TestChunkFile, TestEntry};
use serde_json::{Value, json};

use super::support::{self, TestResult};

#[test]
fn model_companions_keep_observed_paths_ambiguity_selection_and_unbounded_lod_suffixes()
-> TestResult {
    let temp = tempfile::tempdir()?;
    let archive = support::archive_with(temp.path(), fixture())?;
    let output = temp.path().join("out");
    let options = ExportOptions {
        kinds: vec![MediaKind::Model],
        include: Some(Filter::parse("Models/main.cgf")?),
        ..ExportOptions::default()
    };
    let index = support::published(p4kx_media::export(&archive, &output, &options)?)?;
    let rows = support::records(&output, &index, "model.jsonl")?;
    let companions = rows
        .iter()
        .filter(|row| row.subresource.namespace == "model-companion")
        .collect::<Vec<_>>();
    assert_eq!(
        companions.len(),
        7,
        "physical and justified missing sibling paths"
    );
    assert_eq!(
        index.source_entries.written, 1,
        "unselected corrupt targets are never read"
    );
    let base = output.join(&index.artifact_base);
    validate_rows(&companions, &base)?;
    for (name, level, canonical, status) in [
        ("Models/main.cgfm", None, true, "missing"),
        ("Models/Main_lod1.CGFM", Some(1), true, "ambiguous"),
        ("Models/MAIN_LOD1.cgf", Some(1), true, "not-selected"),
        ("Models/main_lod20.cgfm", Some(20), false, "not-selected"),
        ("Models/main_lod20.cgf", Some(20), false, "missing"),
        (
            "Models/main_lod999999999999999.cgf",
            None,
            false,
            "not-selected",
        ),
        (
            "Models/main_lod999999999999999.cgfm",
            None,
            false,
            "missing",
        ),
    ] {
        check_reference(&companions, name, level, canonical, status)?;
    }
    Ok(())
}

fn fixture() -> [TestEntry; 6] {
    let model = TestChunkFile::ivo()
        .chunk(ivo::MESH_INFO.id(), 0x900, [0; 16])
        .bytes();
    [
        TestEntry::stored("Models/main.cgf", model),
        TestEntry::stored("Models/Main_lod1.CGFM", b"unread").with_crc32c(1),
        TestEntry::stored("models/main_LOD1.cgfm", b"unread").with_crc32c(1),
        TestEntry::stored("Models/MAIN_LOD1.cgf", b"unread").with_crc32c(1),
        TestEntry::stored("Models/main_lod20.cgfm", b"unread").with_crc32c(1),
        TestEntry::stored("Models/main_lod999999999999999.cgf", b"unread").with_crc32c(1),
    ]
}

fn check_reference(
    rows: &[&MediaRecord],
    original: &str,
    level: Option<u32>,
    canonical: bool,
    status: &str,
) -> TestResult {
    let row = find(rows, original)?;
    for (path, value) in [
        ("/payload/level", json!(level)),
        ("/payload/canonical_reader_candidate", json!(canonical)),
        ("/payload/reference/resolution/status", json!(status)),
    ] {
        assert_eq!(
            row.data.pointer(path),
            Some(&value),
            "reference {original} at {path}"
        );
    }
    if let Some((_, suffix)) = original.rsplit_once("_lod")
        && let Some((digits, _)) = suffix.split_once('.')
    {
        assert_eq!(
            row.data.pointer("/payload/suffix"),
            Some(&json!(digits)),
            "original suffix spelling"
        );
    }
    Ok(())
}

fn validate_rows(rows: &[&MediaRecord], base: &Path) -> TestResult {
    let schema = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    for row in rows {
        assert!(
            schema.is_valid(&serde_json::to_value(row)?),
            "companion record satisfies its closed schema"
        );
    }
    let text = std::fs::read_to_string(base.join("field-origins.jsonl"))?;
    let rows = text
        .lines()
        .map(serde_json::from_str::<Value>)
        .collect::<Result<Vec<_>, _>>()?;
    let fields = rows
        .iter()
        .filter(|row| {
            row.get("field").and_then(Value::as_str)
                == Some("/data/payload/reference/original/value")
        })
        .collect::<Vec<_>>();
    assert_eq!(
        fields.len(),
        7,
        "each physical reference identifier has a separate origin"
    );
    for field in fields {
        assert_eq!(
            field.get("containsEncryptedSource"),
            Some(&json!(false)),
            "candidate path metadata has plain origin"
        );
        assert_eq!(
            field.get("sourceSetComplete"),
            Some(&json!(true)),
            "all original selector paths have actual source proof"
        );
    }
    Ok(())
}

fn find<'a>(rows: &[&'a MediaRecord], path: &str) -> Result<&'a MediaRecord, Box<dyn Error>> {
    rows.iter()
        .copied()
        .find(|row| {
            row.data
                .pointer("/payload/reference/original/value")
                .and_then(Value::as_str)
                == Some(path)
        })
        .ok_or_else(|| format!("missing exact companion identifier {path}").into())
}
