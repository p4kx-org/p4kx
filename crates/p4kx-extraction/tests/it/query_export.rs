// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Exporting a Query result to a file as CSV or JSON, page by page, replacing the file only once
//! it is complete.

use std::error::Error;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use p4kx_dataforge::DataForge;
use p4kx_dataforge::query::{Query, ResultFormat};
use p4kx_extraction::{CancelToken, QueryExportError, QueryExportOptions, export_query};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

type TestResult = Result<(), Box<dyn Error>>;

fn no_archive() -> PathBuf {
    PathBuf::from("no-such-archive.p4k")
}

fn database() -> Vec<u8> {
    DataForgeBuilder::new()
        .structure(
            TestStruct::new("Item")
                .field("name", TestType::String)
                .field("mass", TestType::Single),
        )
        .record(
            TestRecord::new(1, "Item", "Item.Box", "libs/items/box.xml")
                .value("name", TestValue::text("Box, large"))
                .value("mass", TestValue::Single(2.5)),
        )
        .record(
            TestRecord::new(2, "Item", "Item.Crate", "libs/items/crate.xml")
                .value("name", TestValue::text("Crate"))
                .value("mass", TestValue::Single(-1.0)),
        )
        .build()
}

#[test]
fn csv_is_written_like_the_cli_writes_it() -> TestResult {
    let bytes = database();
    let forge = DataForge::open(&bytes)?;
    let compiled = Query::parse("from Item select name, mass")?.compile(&forge)?;
    let dir = tempfile::tempdir()?;
    let out = dir.path().join("result.csv");

    let written = export_query(
        &compiled,
        &compiled.run()?,
        (&out, ResultFormat::Csv),
        &no_archive(),
        &QueryExportOptions::default(),
    )?;

    assert_eq!(written.records, 2, "rows");
    assert_eq!(written.failed, 0, "failed");
    assert_eq!(
        fs::read_to_string(&out)?,
        "\u{feff}record path,name,mass\r\nlibs/items/box,\"Box, large\",2.5\r\nlibs/items/crate,Crate,-1\r\n",
        "CSV"
    );
    Ok(())
}

#[test]
fn json_is_one_array_of_typed_objects() -> TestResult {
    let bytes = database();
    let forge = DataForge::open(&bytes)?;
    let compiled = Query::parse("from Item select mass where mass > 0")?.compile(&forge)?;
    let dir = tempfile::tempdir()?;
    let out = dir.path().join("result.json");

    export_query(
        &compiled,
        &compiled.run()?,
        (&out, ResultFormat::Json),
        &no_archive(),
        &QueryExportOptions::default(),
    )?;

    assert_eq!(
        fs::read_to_string(&out)?,
        "[{\"record path\":\"libs/items/box\",\"mass\":2.5}]\n",
        "JSON"
    );
    Ok(())
}

#[test]
fn progress_counts_the_rows_written() -> TestResult {
    let bytes = database();
    let forge = DataForge::open(&bytes)?;
    let compiled = Query::parse("from Item")?.compile(&forge)?;
    let dir = tempfile::tempdir()?;
    let seen = AtomicU64::new(0);
    let progress = |rows: u64| seen.store(rows, Ordering::Relaxed);

    export_query(
        &compiled,
        &compiled.run()?,
        (&dir.path().join("result.csv"), ResultFormat::Csv),
        &no_archive(),
        &QueryExportOptions {
            cancel: None,
            progress: Some(&progress),
            failed: None,
        },
    )?;

    assert_eq!(seen.load(Ordering::Relaxed), 2, "all rows");
    Ok(())
}

#[test]
fn a_cancelled_export_keeps_the_existing_file_and_leaves_no_part_file() -> TestResult {
    let bytes = database();
    let forge = DataForge::open(&bytes)?;
    let compiled = Query::parse("from Item")?.compile(&forge)?;
    let dir = tempfile::tempdir()?;
    let out = dir.path().join("result.csv");
    fs::write(&out, "before")?;
    let cancel = CancelToken::new();
    cancel.cancel();

    let result = export_query(
        &compiled,
        &compiled.run()?,
        (&out, ResultFormat::Csv),
        &no_archive(),
        &QueryExportOptions {
            cancel: Some(&cancel),
            progress: None,
            failed: None,
        },
    );

    assert!(
        matches!(result, Err(QueryExportError::Cancelled)),
        "{result:?}"
    );
    assert_eq!(fs::read_to_string(&out)?, "before", "kept");
    assert_eq!(fs::read_dir(dir.path())?.count(), 1, "no part file");
    Ok(())
}

#[test]
fn a_folder_that_does_not_exist_is_a_write_error_naming_the_file() -> TestResult {
    let bytes = database();
    let forge = DataForge::open(&bytes)?;
    let compiled = Query::parse("from Item")?.compile(&forge)?;
    let dir = tempfile::tempdir()?;
    let out = dir.path().join("missing").join("result.csv");

    let result = export_query(
        &compiled,
        &compiled.run()?,
        (&out, ResultFormat::Csv),
        &no_archive(),
        &QueryExportOptions::default(),
    );

    match result {
        Err(QueryExportError::Write { path, .. }) => assert_eq!(path, out, "path"),
        other => return Err(format!("expected a write error: {other:?}").into()),
    }
    Ok(())
}

#[test]
fn an_export_over_the_open_archive_is_refused() -> TestResult {
    let bytes = database();
    let forge = DataForge::open(&bytes)?;
    let compiled = Query::parse("from Item")?.compile(&forge)?;
    let dir = tempfile::tempdir()?;
    let archive = dir.path().join("Data.p4k");
    fs::write(&archive, "archive")?;
    let out = dir.path().join(".").join("Data.p4k");

    let result = export_query(
        &compiled,
        &compiled.run()?,
        (&out, ResultFormat::Csv),
        &archive,
        &QueryExportOptions::default(),
    );

    assert!(
        matches!(result, Err(QueryExportError::OutputIsArchive(_))),
        "{result:?}"
    );
    assert_eq!(fs::read_to_string(&archive)?, "archive", "untouched");
    assert_eq!(fs::read_dir(dir.path())?.count(), 1, "no part file");
    Ok(())
}
