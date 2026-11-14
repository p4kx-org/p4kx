// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::chunks::{crch, ivo};
use p4kx_extraction::{Archive, Filter, OpenOptions};
use p4kx_media::{ExportOptions, Identity, MediaKind, MediaRecord};
use p4kx_testkit::{ArchiveBuilder, TestChunkFile, TestEntry, test_key};
use serde_json::{Value, json};

use super::model_source_provider::field_uses;
use super::support::{self, TestResult};

const NAMESPACE: &str = "model-material-library";

#[test]
fn model_material_references_keep_authored_ranges_unselected_targets_and_owning_input() -> TestResult
{
    let temp = tempfile::tempdir()?;
    let file = ivo_file();
    let archive = archive(temp.path(), &file)?;
    let output = temp.path().join("model-material");
    let index = support::published(p4kx_media::export(
        &archive,
        &output,
        &ExportOptions {
            kinds: vec![MediaKind::Model],
            include: Some(Filter::parse("*.cg*")?),
            ..ExportOptions::default()
        },
    )?)?;
    assert_eq!(
        index.source_entries.written, 3,
        "only physical model entries are opened"
    );
    let rows = support::records(&output, &index, "model.jsonl")?;
    let schema = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    for row in &rows {
        assert!(
            schema.is_valid(&serde_json::to_value(row)?),
            "complete model row schema"
        );
    }
    assert_ivo_rows(&rows, &output.join(&index.artifact_base), &file)?;
    assert_crch_rows(&rows)?;
    assert_opaque_rows(&rows)?;
    let (_, geometry) = positioned(&rows, 2, NAMESPACE, 0)?;
    assert_eq!(
        geometry
            .data
            .pointer("/payload/primary_model_reader_selector"),
        Some(&json!(false)),
        "a physical geometry declaration is not the root model selection",
    );
    Ok(())
}

fn ivo_file() -> TestChunkFile {
    TestChunkFile::ivo()
        .chunk(ivo::MTL_NAME.id(), 0x900, b" library.MTL \0trailing")
        .chunk(ivo::MTL_NAME_320.id(), 0x900, b"/Shared/Paint\0")
        .chunk(ivo::MTL_NAME.id(), 0x900, b"duplicate\0")
        .chunk(ivo::MTL_NAME.id(), 0x900, b"absent\0")
        .chunk(ivo::MTL_NAME.id(), 0x900, b"../invalid\0")
        .chunk(ivo::MTL_NAME.id(), 0x900, b"\xffx\0")
        .chunk(ivo::MTL_NAME.id(), 0x900, b"\0padding")
        .chunk(ivo::MTL_NAME_VARIANT.id(), 0x900, b"unknown\0")
        .chunk(ivo::MTL_NAME.id(), 0x900, b"root_only\0")
}

fn archive(folder: &Path, file: &TestChunkFile) -> Result<Archive, Box<dyn std::error::Error>> {
    let legacy = TestChunkFile::crch()
        .chunk(crch::MTL_NAME.id(), 0x801, b"unsupported")
        .big_endian_chunk(crch::MTL_NAME.id(), 0x802, b"Shared/Paint")
        .bytes();
    let geometry = TestChunkFile::ivo()
        .chunk(ivo::MTL_NAME.id(), 0x900, b"library")
        .bytes();
    let path = folder.join("Data.p4k");
    let mut archive = ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data/Car/model.cga", file.bytes()).encrypted())
        .entry(TestEntry::stored("Data/Other/legacy.cgf", legacy))
        .entry(TestEntry::stored("Data/Car/model.cgam", geometry));
    for path in [
        "Data/Car/Library.mtl",
        "Data/library.mtl",
        "Data/Shared/Paint.mtl",
        "Data/Car/Duplicate.mtl",
        "data/car/duplicate.MTL",
        "Data/duplicate.mtl",
        "Data/root_only.mtl",
        "Data/Car/model.caf",
    ] {
        archive = archive.entry(TestEntry::stored(path, b"unread target").with_crc32c(1));
    }
    archive.write_to(&path)?;
    Ok(Archive::open_with(
        path,
        OpenOptions::new().key(Some(test_key())),
    )?)
}

fn assert_ivo_rows(rows: &[MediaRecord], base: &Path, file: &TestChunkFile) -> TestResult {
    let offsets = file.offsets();
    for (index, token, resolution, target) in [
        (0, " library.MTL ", "not-selected", Some(3)),
        (1, "/Shared/Paint", "not-selected", Some(5)),
        (2, "duplicate", "ambiguous", None),
        (3, "absent", "missing", None),
        (4, "../invalid", "unavailable", None),
        (8, "root_only", "not-selected", Some(9)),
    ] {
        let (record_index, row) = positioned(rows, 0, NAMESPACE, index)?;
        assert_reference(row, token, resolution, target);
        let offset = *offsets.get(index).ok_or("original token offset")?;
        let end = offset + token.len() as u64;
        assert_eq!(
            row.data.pointer("/payload/token"),
            Some(&json!({"basis":{"type":"entry"}, "start":offset, "end_exclusive":end})),
            "physical token extent excludes the terminator and padding",
        );
        assert_eq!(
            row.data.pointer("/payload/primary_model_reader_selector"),
            Some(&json!(index == 1)),
            "IVO precedence is identical to the conversion reader",
        );
        let evidence = field_uses(
            base,
            record_index,
            "/data/payload/reference/original/value",
            0,
            true,
        )?;
        let exact = json!({
            "type":"subresource", "namespace":"model-byte-range",
            "indices":[offset.to_string(), end.to_string()], "field":"modelMaterialName"
        });
        assert_eq!(
            evidence.first().and_then(|usage| usage.get("location")),
            Some(&exact),
            "field is tagged using the encrypted model's exact token bytes",
        );
        let (_, directory) = positioned(rows, 0, "model-chunk", index)?;
        assert_eq!(
            directory.data.pointer("/payload/semantic_record"),
            Some(&json!(NAMESPACE)),
            "physical table row identifies its typed counterpart",
        );
    }
    Ok(())
}

fn assert_opaque_rows(rows: &[MediaRecord]) -> TestResult {
    for (index, disposition) in [(5, "non-utf8"), (6, "empty")] {
        let (_, row) = positioned(rows, 0, NAMESPACE, index)?;
        assert_eq!(
            row.data.pointer("/payload/disposition"),
            Some(&json!(disposition)),
            "undecodable or empty strings have explicit disposition"
        );
        assert_eq!(
            row.data.pointer("/payload/reference"),
            None,
            "no manufactured target"
        );
    }
    let (_, unknown) = positioned(rows, 0, "model-chunk", 7)?;
    assert_eq!(
        unknown.data.pointer("/payload/semantic_record"),
        None,
        "unmeasured type stays opaque"
    );
    assert!(
        rows.iter()
            .all(|row| !row.subresource.namespace.contains("animation")),
        "same-basename CAF is not an authored model binding"
    );
    Ok(())
}

fn assert_crch_rows(rows: &[MediaRecord]) -> TestResult {
    let (_, unsupported) = positioned(rows, 1, NAMESPACE, 0)?;
    assert_eq!(
        unsupported.data.pointer("/payload/disposition"),
        Some(&json!("unsupported-version")),
        "CrCh version must use the supported name layout",
    );
    let (_, row) = positioned(rows, 1, NAMESPACE, 1)?;
    assert_reference(row, "Shared/Paint", "not-selected", Some(5));
    assert_eq!(
        row.data.pointer("/payload/primary_model_reader_selector"),
        Some(&json!(true)),
        "supported CrCh name is the conversion reader's selector",
    );
    Ok(())
}

fn assert_reference(row: &MediaRecord, token: &str, resolution: &str, target: Option<u64>) {
    for (pointer, expected) in [
        ("/payload/disposition", json!("named")),
        ("/payload/reference/original/value", json!(token)),
        ("/payload/reference/resolution/status", json!(resolution)),
    ] {
        assert_eq!(
            row.data.pointer(pointer),
            Some(&expected),
            "source declaration field {pointer}"
        );
    }
    if let Some(target) = target {
        for (pointer, expected) in [
            ("/payload/reference/resolution/reason", json!("filter")),
            (
                "/payload/reference/resolution/target/kind",
                json!("material"),
            ),
            (
                "/payload/reference/resolution/target/source/entry/entry_index",
                json!(target),
            ),
        ] {
            assert_eq!(
                row.data.pointer(pointer),
                Some(&expected),
                "exact target field {pointer}"
            );
        }
    }
    if resolution == "unavailable" {
        assert_eq!(
            row.data
                .pointer("/payload/reference/resolution/reason/type"),
            Some(&json!("invalid-identifier")),
            "lexically invalid original is retained without a directory lookup",
        );
    }
    if resolution == "ambiguous" {
        assert_eq!(
            row.data
                .pointer("/payload/reference/resolution/candidates")
                .and_then(Value::as_array)
                .map(Vec::len),
            Some(2),
            "ambiguous sibling never falls through to the Data root",
        );
    }
}

fn positioned<'a>(
    rows: &'a [MediaRecord],
    owner: usize,
    namespace: &str,
    index: usize,
) -> Result<(usize, &'a MediaRecord), Box<dyn std::error::Error>> {
    rows.iter()
        .enumerate()
        .find(|(_, row)| {
            matches!(&row.source.identity, Identity::Entry { entry, .. } if entry.entry_index == owner)
                && row.subresource.namespace == namespace
                && row.subresource.indices == [index as u64]
        })
        .ok_or_else(|| format!("missing exact {namespace} row {owner}/{index}").into())
}
