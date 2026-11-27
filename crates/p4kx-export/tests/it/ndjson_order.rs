// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The record-path order of the NDJSON Record export.

use std::error::Error;
use std::fs;
use std::io::{self, Write as _};
use std::path::Path;

use p4kx_export::{
    NDJSON_FILE, RecordExportOptions, RecordForm, write_ndjson_with_origins, write_record_export,
};
use p4kx_extraction::{
    Archive, DataForge, DataForgeEntry, JsonLayout, OriginArtifactBinding, OriginDigestWriter,
    RecordJson, RecordStreamInput, RecordStreamRows, RecordStreamSpool, TreeBuilder,
};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestRecord, TestStruct, TestType, TestValue,
};
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn Error>>;

fn ndjson_numbers(builder: &DataForgeBuilder) -> Result<Vec<u64>, Box<dyn Error>> {
    let bytes = builder.build();
    let forge = DataForge::open(&bytes)?;
    let records: Vec<_> = forge.records().collect();
    let dir = tempfile::tempdir()?;
    let summary = write_record_export(
        &forge,
        &records,
        RecordForm::Ndjson,
        dir.path(),
        &RecordExportOptions::default(),
    );
    assert!(summary.is_complete(), "{:?}", summary.failures());
    let text = fs::read_to_string(dir.path().join(NDJSON_FILE))?;
    let numbers = text
        .lines()
        .map(|line| {
            serde_json::from_str::<Value>(line)
                .map(|value| value.get("n").and_then(Value::as_u64).unwrap_or(u64::MAX))
        })
        .collect::<Result<_, _>>()?;
    Ok(numbers)
}

fn items(paths: &[(&str, &str)]) -> DataForgeBuilder {
    (0_u32..).zip(paths).fold(
        DataForgeBuilder::new().structure(TestStruct::new("Item").field("n", TestType::UInt32)),
        |builder, (n, (name, path))| {
            builder.record(
                TestRecord::new(u128::from(n) + 1, "Item", *name, *path)
                    .value("n", TestValue::UInt32(n)),
            )
        },
    )
}

#[test]
fn record_paths_compare_ignoring_ascii_case_and_separators_then_names() -> TestResult {
    let builder = items(&[
        ("Item.B", "libs/Zeta.xml"),
        ("Item.A", "libs/alpha.xml"),
        ("Item.C", "LIBS\\beta.xml"),
        ("Item.B", "libs/Alpha.xml"),
        ("Item.A", "libs/zeta.xml"),
    ]);

    let numbers = ndjson_numbers(&builder)?;

    assert_eq!(
        numbers,
        [1, 3, 2, 4, 0],
        "alpha (A, then B), beta, zeta (A, then B)"
    );
    Ok(())
}
#[test]
fn ndjson_keeps_record_path_order_across_batches() -> TestResult {
    let count = 5_000_u32;
    let builder = (0..count).fold(
        DataForgeBuilder::new().structure(TestStruct::new("Item").field("n", TestType::UInt32)),
        |builder, n| {
            builder.record(
                TestRecord::new(
                    u128::from(n) + 1,
                    "Item",
                    format!("Item.{n}"),
                    format!("items/{:05}.xml", count - n),
                )
                .value("n", TestValue::UInt32(n)),
            )
        },
    );
    let numbers = ndjson_numbers(&builder)?;
    let expected: Vec<u64> = (0..u64::from(count)).rev().collect();
    assert_eq!(
        numbers, expected,
        "sorted by record path, not by definition"
    );
    Ok(())
}

#[test]
fn ndjson_origin_lines_follow_successful_rows_across_worker_parts() -> TestResult {
    const COUNT: u32 = 65;
    let bytes = (0..COUNT)
        .fold(
            DataForgeBuilder::new().structure(TestStruct::new("Item").field("n", TestType::UInt32)),
            |builder, n| {
                builder.record(
                    TestRecord::new(
                        u128::from(n) + 1,
                        "Item",
                        format!("Item.{n}"),
                        format!("items/{:05}.xml", COUNT - n),
                    )
                    .value("n", TestValue::UInt32(n)),
                )
            },
        )
        .build();
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data\\Game2.dcb", bytes.clone()))
        .write_to(&path)?;
    let archive = Archive::open(&path)?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let foreign = DataForge::open(&bytes)?;
    let repeated = forge.records().next().ok_or("first original Record")?;
    let mut selected: Vec<_> = forge.records().collect();
    selected.extend(foreign.records().take(1));
    selected.push(repeated);

    let summary = write_ndjson_with_origins(
        &entry,
        &forge,
        &selected,
        dir.path(),
        &RecordExportOptions::default(),
    );
    assert_eq!(
        summary.written(),
        COUNT as usize,
        "duplicate selection and foreign Record add no lines"
    );
    assert_eq!(
        summary.failures().len(),
        1,
        "the foreign Record was reported"
    );
    assert!(summary.error().is_none(), "the file committed");
    assert!(
        summary.origin_failures().is_empty(),
        "the sidecar committed"
    );
    let payload = fs::read_to_string(dir.path().join(NDJSON_FILE))?;
    let proof = summary.origin_receipts().first().ok_or("companion path")?;
    check_part_positions(&payload, &fs::read_to_string(proof)?)?;
    Ok(())
}

fn check_part_positions(payload: &str, proof: &str) -> TestResult {
    let lines: Vec<Value> = payload
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()?;
    let proof: Vec<Value> = proof
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()?;
    for (line, index) in [(0, 64), (63, 1), (64, 0)] {
        let expected = json!(index);
        let line_text = line.to_string();
        assert_eq!(
            lines.get(line).and_then(|row| row.get("n")),
            Some(&expected),
            "physical line {line} holds Record {index}"
        );
        let field = proof
            .iter()
            .find(|row| {
                row.pointer("/address/line").and_then(Value::as_str) == Some(line_text.as_str())
                    && row.pointer("/address/path").and_then(Value::as_str) == Some("/$guid")
            })
            .ok_or("physical Record GUID field")?;
        let uses = field
            .get("sourceUsesRefs")
            .and_then(Value::as_array)
            .ok_or("source uses")?;
        assert!(
            proof
                .iter()
                .filter_map(|row| row.get("sourceUse"))
                .any(|row| {
                    row.get("id").is_some_and(|id| uses.contains(id))
                        && row.pointer("/location/recordIndex") == Some(&expected)
                }),
            "line {line} refers to original index {index}"
        );
    }
    Ok(())
}

fn ndjson_entry(path: &Path, bytes: &[u8]) -> Result<DataForgeEntry, Box<dyn Error>> {
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data\\Game2.dcb", bytes.to_vec()))
        .write_to(path)?;
    Ok(DataForgeEntry::read(&Archive::open(path)?)?)
}

#[test]
fn ndjson_spool_rejects_identical_data_from_a_foreign_original_entry() -> TestResult {
    let root = tempfile::tempdir()?;
    let bytes = items(&[("Item.A", "items/a.xml")]).build();
    let entry = ndjson_entry(&root.path().join("A.p4k"), &bytes)?;
    let foreign_entry = ndjson_entry(&root.path().join("B.p4k"), &bytes)?;
    let forge = entry.open()?;
    let foreign_forge = foreign_entry.open()?;
    assert_eq!(
        RecordStreamInput::new(&entry, &foreign_forge)
            .err()
            .map(|error| error.kind()),
        Some(io::ErrorKind::InvalidInput),
        "equal bytes cannot be bound to the foreign Entry"
    );
    let input = RecordStreamInput::new(&entry, &forge)?;
    let foreign_input = RecordStreamInput::new(&foreign_entry, &foreign_forge)?;
    let original = forge.records().next().ok_or("original Record")?;
    let original = TreeBuilder::new().build_positioned_partial(&original)?;
    let foreign = foreign_forge.records().next().ok_or("foreign Record")?;
    let foreign = TreeBuilder::new().build_positioned_partial(&foreign)?;
    let mut writer = RecordJson::new(JsonLayout::Compact);
    let mut file = Vec::new();
    assert_eq!(
        RecordStreamRows::write_json(input, &mut writer, &foreign, &mut file, None, None)
            .err()
            .map(|error| error.kind()),
        Some(io::ErrorKind::InvalidInput),
        "a bound writer rejects a foreign tree"
    );
    assert!(file.is_empty(), "the rejected row contributes no bytes");
    let local_row =
        RecordStreamRows::write_json(input, &mut writer, &original, &mut file, None, None)?;
    let foreign_row = RecordStreamRows::write_json(
        foreign_input,
        &mut writer,
        &foreign,
        &mut Vec::new(),
        None,
        None,
    )?;
    let mut spool = RecordStreamSpool::new(input)?;
    assert_eq!(
        spool
            .append_part(vec![local_row, foreign_row], 0)
            .err()
            .map(|error| error.kind()),
        Some(io::ErrorKind::InvalidInput),
        "a mixed part is rejected before staging any of its fields"
    );

    file.clear();
    let row = RecordStreamRows::write_json(input, &mut writer, &original, &mut file, None, None)?;
    spool.append_part(vec![row], 0)?;
    assert_first_line(&file, spool)?;
    Ok(())
}

fn assert_first_line(file: &[u8], spool: RecordStreamSpool<'_>) -> TestResult {
    let mut measured = OriginDigestWriter::new(io::sink());
    measured.write_all(file)?;
    let binding = OriginArtifactBinding::Stdout {
        digest: measured.finish()?,
    };
    let mut out = Vec::new();
    spool.finish()?.write(&mut out, &binding)?;
    let rows = str::from_utf8(&out)?
        .lines()
        .map(serde_json::from_str::<Value>)
        .collect::<Result<Vec<_>, _>>()?;
    let fields: Vec<_> = rows
        .iter()
        .filter_map(|row| row.pointer("/address/line"))
        .collect();
    assert!(!fields.is_empty(), "the admitted row produces observations");
    assert!(
        fields.iter().all(|line| line.as_str() == Some("0")),
        "the refused part advances no line"
    );
    Ok(())
}
