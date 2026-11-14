// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Writing Records of a DataForge into one combined Record XML document.

use std::error::Error;
use std::fs;

use p4kx_extraction::{
    CancelToken, ConflictPolicy, DataForge, RecordExportError, RecordExportOptions, export_combined,
};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

type TestResult = Result<(), Box<dyn Error>>;

fn builder(records: impl IntoIterator<Item = TestRecord>) -> DataForgeBuilder {
    records.into_iter().fold(
        DataForgeBuilder::new().structure(TestStruct::new("Item").field("count", TestType::Int32)),
        DataForgeBuilder::record,
    )
}

/// Records whose definition order is the reverse of their record-path order, enough of them to
/// span several batches.
fn many(count: u32) -> Vec<u8> {
    builder((0..count).rev().map(|number| {
        TestRecord::new(
            u128::from(number) + 1,
            "Item",
            format!("Item.N{number:05}"),
            format!("records/n{number:05}.xml"),
        )
        .value(
            "count",
            TestValue::Int32(i32::try_from(number).unwrap_or_default()),
        )
    }))
    .build()
}

#[test]
fn records_spanning_many_batches_are_written_in_record_path_order() -> TestResult {
    let bytes = many(2500);
    let forge = DataForge::open(&bytes)?;
    let records: Vec<_> = forge.records().collect();
    let dir = tempfile::tempdir()?;
    let file = dir.path().join("all.xml");

    let report = export_combined(&forge, &records, &file, &RecordExportOptions::default())?;

    assert_eq!(report.written(), 2500, "every Record: {report:?}");
    let xml = fs::read_to_string(&file)?;
    let document = roxmltree::Document::parse(&xml)?;
    let paths: Vec<_> = document
        .root_element()
        .children()
        .filter_map(|node| node.attribute("path"))
        .collect();
    let expected: Vec<_> = (0..2500)
        .map(|number| format!("records/n{number:05}"))
        .collect();
    assert_eq!(
        paths, expected,
        "ordered by record path, not definition order"
    );
    let again = dir.path().join("again.xml");
    let reversed: Vec<_> = records.iter().rev().copied().collect();
    export_combined(&forge, &reversed, &again, &RecordExportOptions::default())?;
    assert_eq!(
        fs::read(&again)?,
        xml.as_bytes(),
        "the selection order does not matter"
    );
    Ok(())
}

#[test]
fn a_record_selected_twice_is_written_once() -> TestResult {
    let bytes = many(3);
    let forge = DataForge::open(&bytes)?;
    let records: Vec<_> = forge.records().chain(forge.records()).collect();
    let dir = tempfile::tempdir()?;

    let report = export_combined(
        &forge,
        &records,
        &dir.path().join("all.xml"),
        &RecordExportOptions::default(),
    )?;

    assert_eq!(report.written(), 3, "each Record once: {report:?}");
    Ok(())
}

#[test]
fn an_empty_selection_gives_an_empty_root_element() -> TestResult {
    let bytes = many(1);
    let forge = DataForge::open(&bytes)?;
    let dir = tempfile::tempdir()?;
    let file = dir.path().join("none.xml");

    let report = export_combined(&forge, &[], &file, &RecordExportOptions::default())?;

    assert_eq!(report.written(), 0, "nothing selected");
    assert_eq!(
        fs::read_to_string(&file)?,
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <p4kx:records xmlns:p4kx=\"https://github.com/p4kx-org/p4kx/record-xml\"/>\n",
        "a self-closing root"
    );
    Ok(())
}

#[test]
fn a_record_of_another_dataforge_is_reported_and_left_out() -> TestResult {
    let bytes = many(2);
    let forge = DataForge::open(&bytes)?;
    let other_bytes = many(1);
    let other = DataForge::open(&other_bytes)?;
    let records: Vec<_> = forge.records().chain(other.records()).collect();
    let dir = tempfile::tempdir()?;

    let report = export_combined(
        &forge,
        &records,
        &dir.path().join("all.xml"),
        &RecordExportOptions::default(),
    )?;

    assert_eq!(
        report.written(),
        2,
        "the Records of the DataForge: {report:?}"
    );
    assert!(
        matches!(
            report.failures(),
            [failure] if matches!(failure.error(), RecordExportError::OtherDataForge)
        ),
        "the foreign Record fails: {report:?}"
    );
    Ok(())
}

#[test]
fn a_cancelled_run_leaves_no_file_and_keeps_an_existing_one() -> TestResult {
    let bytes = many(50);
    let forge = DataForge::open(&bytes)?;
    let records: Vec<_> = forge.records().collect();
    let dir = tempfile::tempdir()?;
    let file = dir.path().join("all.xml");
    let cancel = CancelToken::new();
    let source_root = tempfile::tempdir()?;
    let source = source_root.path().join("source.dcb");
    fs::write(&source, &bytes)?;
    let protection = p4kx_extraction::ArchiveWriteProtection::new(&source, cancel.flag())?;
    cancel.cancel();
    let options = RecordExportOptions {
        protection: Some(&protection),
        policy: ConflictPolicy::Overwrite,
        cancel: Some(&cancel),
        ..RecordExportOptions::default()
    };

    let fresh = export_combined(&forge, &records, &file, &options)?;
    fs::write(&file, "before")?;
    let existing = export_combined(&forge, &records, &file, &options)?;

    assert_eq!(
        fresh.cancelled(),
        50,
        "every Record is cancelled: {fresh:?}"
    );
    assert_eq!(fresh.written(), 0, "nothing is written: {fresh:?}");
    assert!(!fresh.is_complete(), "a cancelled run is not complete");
    assert_eq!(existing.cancelled(), 50, "cancelled again: {existing:?}");
    assert_eq!(fs::read(&file)?, b"before", "the existing file is kept");
    let names: Vec<_> = fs::read_dir(dir.path())?
        .map(|entry| entry.map(|entry| entry.file_name()))
        .collect::<Result<_, _>>()?;
    assert_eq!(names, ["all.xml"], "no temporary file remains");
    Ok(())
}

#[test]
fn a_folder_in_the_way_is_an_error() -> TestResult {
    let bytes = many(1);
    let forge = DataForge::open(&bytes)?;
    let records: Vec<_> = forge.records().collect();
    let dir = tempfile::tempdir()?;
    let file = dir.path().join("all.xml");
    fs::create_dir(&file)?;

    let result = export_combined(&forge, &records, &file, &RecordExportOptions::default());

    assert!(result.is_err(), "the folder is not replaced: {result:?}");
    assert!(file.is_dir(), "the folder stays");
    Ok(())
}

#[test]
fn records_sharing_a_record_path_are_ordered_by_name_then_definition_order() -> TestResult {
    let item = |guid: u128, name: &str, file: &str| {
        TestRecord::new(guid, "Item", name, file).value("count", TestValue::Int32(1))
    };
    let bytes = builder([
        item(1, "Item.B", "records/shared.xml"),
        item(2, "item.A", "Records\\Shared.xml"),
        item(3, "Item.B", "records/shared.xml"),
        item(4, "Item.C", "records/a.xml"),
    ])
    .build();
    let forge = DataForge::open(&bytes)?;
    let records: Vec<_> = forge.records().collect();
    let dir = tempfile::tempdir()?;
    let file = dir.path().join("all.xml");

    export_combined(&forge, &records, &file, &RecordExportOptions::default())?;

    let xml = fs::read_to_string(&file)?;
    let document = roxmltree::Document::parse(&xml)?;
    let guids: Vec<_> = document
        .root_element()
        .children()
        .filter_map(|node| node.attribute("guid"))
        .map(|guid| guid.trim_start_matches(['0', '-']).to_owned())
        .collect();
    assert_eq!(
        guids,
        ["4", "2", "1", "3"],
        "record path and name ignoring case, then definition order"
    );
    Ok(())
}

#[test]
fn every_record_of_a_combined_export_is_reported_as_it_ends() -> TestResult {
    let bytes = many(1500);
    let forge = DataForge::open(&bytes)?;
    let records: Vec<_> = forge.records().collect();
    let dir = tempfile::tempdir()?;
    let ended = std::sync::atomic::AtomicUsize::new(0);
    let progress = || {
        ended.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    };
    let options = RecordExportOptions {
        progress: Some(&progress),
        ..RecordExportOptions::default()
    };

    export_combined(&forge, &records, &dir.path().join("all.xml"), &options)?;

    assert_eq!(
        ended.load(std::sync::atomic::Ordering::Relaxed),
        1500,
        "once per Record"
    );
    Ok(())
}
