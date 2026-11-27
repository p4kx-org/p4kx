// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The flat value Record export: files, order, conflicts and protection.

use std::error::Error;
use std::fs;

use p4kx_export::{
    ConflictPolicy, FLAT_CSV_FILE, FLAT_NDJSON_FILE, RecordExportOptions, RecordForm,
    write_record_export,
};
use p4kx_extraction::{DataForge, ExtractionError, protect};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

type TestResult = Result<(), Box<dyn Error>>;

fn items(count: u32) -> DataForgeBuilder {
    (0..count).fold(
        DataForgeBuilder::new().structure(
            TestStruct::new("Item")
                .field("n", TestType::UInt32)
                .field("name", TestType::String),
        ),
        |builder, n| {
            builder.record(
                TestRecord::new(
                    u128::from(n) + 1,
                    "Item",
                    format!("Item.{n:05}"),
                    format!("items/{n:05}.xml"),
                )
                .value("n", TestValue::UInt32(n))
                .value("name", TestValue::text("")),
            )
        },
    )
}

fn export(
    builder: &DataForgeBuilder,
    form: RecordForm,
    options: &RecordExportOptions<'_>,
) -> Result<(tempfile::TempDir, p4kx_export::RecordExportSummary), Box<dyn Error>> {
    let bytes = builder.build();
    let forge = DataForge::open(&bytes)?;
    let records: Vec<_> = forge.records().collect();
    let dir = tempfile::tempdir()?;
    let summary = write_record_export(&forge, &records, form, dir.path(), options);
    Ok((dir, summary))
}

#[test]
fn flat_csv_is_one_file_with_a_row_per_value_in_record_path_order() -> TestResult {
    let count = 5_000_u32;

    let (dir, summary) = export(
        &items(count),
        RecordForm::FlatCsv,
        &RecordExportOptions::default(),
    )?;

    assert!(summary.is_complete(), "{:?}", summary.failures());
    assert_eq!(summary.written(), count as usize);
    let bytes = fs::read(dir.path().join(FLAT_CSV_FILE))?;
    assert!(bytes.starts_with(b"\xEF\xBB\xBFrecord_guid,"));
    let text = String::from_utf8(bytes)?;
    let rows: Vec<&str> = text.split("\r\n").filter(|row| !row.is_empty()).collect();
    assert_eq!(
        rows.len(),
        count as usize + 1,
        "header and one row per Record"
    );
    assert!(rows.get(1).is_some_and(|row| row.ends_with(",n,,uint,0,,")));
    assert!(
        rows.last()
            .is_some_and(|row| row.ends_with(",n,,uint,4999,,"))
    );
    assert!(!dir.path().join(FLAT_NDJSON_FILE).exists());
    Ok(())
}

#[test]
fn raw_flat_csv_and_flat_ndjson_have_their_own_shapes() -> TestResult {
    let (raw, _) = export(
        &items(2),
        RecordForm::FlatCsvRaw,
        &RecordExportOptions::default(),
    )?;
    let (ndjson, summary) = export(
        &items(2),
        RecordForm::FlatNdjson,
        &RecordExportOptions::default(),
    )?;

    assert!(fs::read(raw.path().join(FLAT_CSV_FILE))?.starts_with(b"record_guid,"));
    assert!(summary.is_complete());
    let text = fs::read_to_string(ndjson.path().join(FLAT_NDJSON_FILE))?;
    assert_eq!(text.lines().count(), 2);
    assert!(
        text.lines()
            .all(|line| line.starts_with("{\"record_guid\":"))
    );
    Ok(())
}

#[test]
fn strict_flat_csv_rejects_invalid_fields_without_publishing_a_partial_export() -> TestResult {
    let bytes = items(1)
        .record(
            TestRecord::new(2, "Item", "Item.invalid", "items/99999.xml")
                .value("n", TestValue::UInt32(2))
                .value("name", TestValue::text("text\twith tab")),
        )
        .build();
    let forge = DataForge::open(&bytes)?;
    let records: Vec<_> = forge.records().collect();
    let dir = tempfile::tempdir()?;
    let file = dir.path().join(FLAT_CSV_FILE);
    fs::write(&file, "old")?;

    let summary = write_record_export(
        &forge,
        &records,
        RecordForm::FlatCsvStrict,
        dir.path(),
        &RecordExportOptions {
            policy: ConflictPolicy::Overwrite,
            ..RecordExportOptions::default()
        },
    );

    assert!(
        matches!(summary.error(), Some(ExtractionError::Write { source, .. })
            if source.kind() == std::io::ErrorKind::InvalidData),
        "the illegal field becomes a file-level format error"
    );
    assert_eq!(summary.written(), 0, "no partial output is counted");
    assert_eq!(fs::read_to_string(file)?, "old", "the prior CSV survives");
    Ok(())
}

#[test]
fn a_kept_file_is_skipped_by_the_conflict_policy() -> TestResult {
    let bytes = items(3).build();
    let forge = DataForge::open(&bytes)?;
    let records: Vec<_> = forge.records().collect();
    let dir = tempfile::tempdir()?;
    fs::write(dir.path().join(FLAT_CSV_FILE), "old")?;
    let options = RecordExportOptions {
        policy: ConflictPolicy::Skip,
        ..RecordExportOptions::default()
    };

    let summary = write_record_export(&forge, &records, RecordForm::FlatCsv, dir.path(), &options);

    assert_eq!(summary.skipped(), [FLAT_CSV_FILE]);
    assert_eq!(fs::read_to_string(dir.path().join(FLAT_CSV_FILE))?, "old");
    Ok(())
}

#[test]
fn a_flat_export_is_refused_inside_a_protected_folder() -> TestResult {
    let bytes = items(1).build();
    let forge = DataForge::open(&bytes)?;
    let records: Vec<_> = forge.records().collect();
    let dir = tempfile::tempdir()?;
    let installation = dir.path().join("StarCitizen");
    let target = installation.join("LIVE").join("export");
    fs::create_dir_all(&installation)?;
    protect(&installation);

    let summary = write_record_export(
        &forge,
        &records,
        RecordForm::FlatCsv,
        &target,
        &RecordExportOptions::default(),
    );

    assert!(matches!(
        summary.error(),
        Some(ExtractionError::Installation(_))
    ));
    assert!(!target.exists());
    Ok(())
}

#[test]
fn forms_name_their_files() {
    assert_eq!(RecordForm::Xml.file(), None);
    assert_eq!(RecordForm::FlatCsvRaw.file(), Some(FLAT_CSV_FILE));
    assert!(RecordForm::FlatNdjson.is_flat());
    assert!(!RecordForm::Ndjson.is_flat());
}
