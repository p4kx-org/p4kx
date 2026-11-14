// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! `p4kx dcb query --out` writes CSV and JSON byte for byte as the window's export does, since
//! both go through `p4kx_extraction::export_query`.

use std::error::Error;
use std::fs;

use p4kx_extraction::query::{Query, ResultFormat};
use p4kx_extraction::{Archive, DataForgeEntry, QueryExportOptions, export_query};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestRecord, TestStruct, TestType, TestValue,
};

type TestResult = Result<(), Box<dyn Error>>;

const QUERY: &str = "at \"*\" select name, mass, tags[*]";

fn database() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .structure(
            TestStruct::new("Item")
                .field("name", TestType::String)
                .field("mass", TestType::Single)
                .array("tags", TestType::Int32),
        )
        .record(
            TestRecord::new(1, "Item", "Item.Box", "libs/items/box.xml")
                .value("name", TestValue::text("Box, \"large\""))
                .value("mass", TestValue::Single(2.5))
                .value(
                    "tags",
                    TestValue::Array(vec![TestValue::Int32(1), TestValue::Int32(2)]),
                ),
        )
        .record(
            TestRecord::new(2, "Item", "Item.Crate", "libs/items/crate.xml")
                .value("name", TestValue::text("two\nlines"))
                .value("mass", TestValue::Single(f32::NAN))
                .value("tags", TestValue::Array(Vec::new())),
        )
}

#[test]
fn the_cli_and_the_export_write_the_same_bytes() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = dir.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data\\Game2.dcb", database().build()))
        .write_to(&archive)?;
    let opened = Archive::open_with_key(&archive, Some(p4kx_testkit::test_key()))?;
    let entry = DataForgeEntry::read(&opened)?;
    let forge = entry.open()?;
    let compiled = Query::parse(QUERY)?.compile(&forge)?;

    for (name, format) in [("csv", ResultFormat::Csv), ("json", ResultFormat::Json)] {
        let cli = dir.path().join(format!("cli.{name}"));
        let window = dir.path().join(format!("window.{name}"));
        let output = p4kx_testkit::p4kx()?
            .args(["dcb", "query"])
            .arg(&archive)
            .arg(QUERY)
            .args(["--format", name, "--out"])
            .arg(&cli)
            .output()?;
        export_query(
            &compiled,
            &compiled.run()?,
            (&window, format),
            &archive,
            &QueryExportOptions::default(),
        )?;

        assert_eq!(output.status.code(), Some(0), "{output:?}");
        assert_eq!(fs::read(&cli)?, fs::read(&window)?, "{name}");
    }
    Ok(())
}
