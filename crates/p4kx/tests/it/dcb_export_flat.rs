// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! `p4kx dcb export --flat` end to end, over a synthetic Archive.

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, GlobalIniBuilder, TestEntry, TestRecord, TestStruct,
    TestType, TestValue,
};

#[path = "dcb_export_flat_origins.rs"]
mod origins;

type TestResult = Result<(), Box<dyn Error>>;

const CART: u128 = 0xcbee_34fd_6441_4000_b99f_924f_7f26_e19f;
const TRUCK: u128 = 0x50e8_a9f6_f956_4000_8ace_7c0c_a9a9_1f5d;

fn write_archive(dir: &Path) -> Result<PathBuf, Box<dyn Error>> {
    write_archive_with_text(dir, "Cart")
}

fn write_archive_with_text(dir: &Path, text: &str) -> Result<PathBuf, Box<dyn Error>> {
    let path = dir.join("Data.p4k");
    let database = DataForgeBuilder::new()
        .structure(
            TestStruct::new("Vehicle")
                .field("name", TestType::Locale)
                .field("mass", TestType::Single)
                .field("maker", TestType::Reference)
                .array("labels", TestType::String),
        )
        .record(
            TestRecord::new(CART, "Vehicle", "Vehicle.Cart", "libs/vehicles/cart.xml")
                .value("name", TestValue::text("@vehicle_Cart"))
                .value("mass", TestValue::Single(0.1))
                .value("maker", TestValue::Reference(TRUCK))
                .value(
                    "labels",
                    TestValue::Array(vec![TestValue::text("=a"), TestValue::text("b")]),
                ),
        )
        .record(
            TestRecord::new(TRUCK, "Vehicle", "Vehicle.Truck", "libs/vehicles/truck.xml")
                .value("name", TestValue::text("")),
        )
        .build();
    let english = GlobalIniBuilder::new().text("vehicle_Cart", text).build();
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data\\Game2.dcb", database))
        .entry(TestEntry::zstd(
            "Data\\Localization\\english\\global.ini",
            english,
        ))
        .write_to(&path)?;
    Ok(path)
}

fn export(args: &[&str], code: i32) -> Result<(tempfile::TempDir, String), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;
    let output = p4kx_testkit::p4kx()?
        .args(["dcb", "export"])
        .arg(&archive)
        .arg("--out")
        .arg(dir.path().join("out"))
        .args(args)
        .output()?;
    assert_eq!(output.status.code(), Some(code), "exit code: {output:?}");
    let text = String::from_utf8_lossy(&output.stdout).into_owned()
        + &String::from_utf8_lossy(&output.stderr);
    Ok((dir, text))
}

#[test]
fn flat_writes_values_csv_by_default() -> TestResult {
    let (dir, _) = export(&["--flat"], 0)?;

    let bytes = fs::read(dir.path().join("out").join("values.csv"))?;
    assert!(bytes.starts_with(b"\xEF\xBB\xBFrecord_guid,"));
    let text = String::from_utf8(bytes)?;
    assert!(text.contains(
        "cbee34fd-6441-4000-b99f-924f7f26e19f,Vehicle.Cart,libs/vehicles/cart,mass,,single,0.1,,\r\n"
    ));
    assert!(text.contains(",labels,0,string,'=a,,\r\n"));
    assert!(text.contains(
        ",maker,,reference,50e8a9f6-f956-4000-8ace-7c0ca9a91f5d,libs/vehicles/truck,\r\n"
    ));
    assert!(
        !text.contains("Vehicle.Truck,libs/vehicles/truck,name"),
        "its name is empty"
    );
    Ok(())
}

#[test]
fn csv_raw_keeps_the_exact_bytes() -> TestResult {
    let (dir, _) = export(&["--flat", "--format", "csv", "--csv-raw"], 0)?;

    let text = fs::read_to_string(dir.path().join("out").join("values.csv"))?;
    assert!(text.starts_with("record_guid,"));
    assert!(text.contains(",labels,0,string,=a,,\r\n"));
    Ok(())
}

#[test]
fn strict_csv_is_available_through_the_flat_export_command() -> TestResult {
    let (dir, _) = export(&["--flat", "--format", "csv-strict"], 0)?;
    let text = fs::read_to_string(dir.path().join("out").join("values.csv"))?;
    assert!(text.starts_with("record_guid,"), "no byte order mark");
    assert!(
        text.contains(",labels,0,string,=a,\"\",\"\"\r\n"),
        "verbatim value and quoted empty fields"
    );
    Ok(())
}

#[test]
fn ndjson_writes_values_ndjson() -> TestResult {
    let (dir, _) = export(&["--flat", "--format", "ndjson"], 0)?;

    let text = fs::read_to_string(dir.path().join("out").join("values.ndjson"))?;
    assert!(text.lines().count() >= 4);
    assert!(text.contains(
        "\"field_path\":\"labels\",\"array_indices\":\"1\",\"kind\":\"string\",\"value\":\"b\""
    ));
    Ok(())
}

#[test]
fn a_language_adds_the_text_column() -> TestResult {
    let (dir, _) = export(&["--flat", "--lang", "en"], 0)?;

    let text = fs::read_to_string(dir.path().join("out").join("values.csv"))?;
    assert!(text.contains(",target_path,text,record_tag\r\n"));
    assert!(
        text.contains(",name,,locale,'@vehicle_Cart,,Cart,\r\n"),
        "{text}"
    );
    Ok(())
}

#[test]
fn a_filter_selects_the_records() -> TestResult {
    let (dir, _) = export(&["--flat", "--filter", "*/truck"], 0)?;

    let text = fs::read_to_string(dir.path().join("out").join("values.csv"))?;
    assert_eq!(
        text.lines().count(),
        2,
        "the header and the one value: {text}"
    );
    assert!(!text.contains("Vehicle.Cart"));
    Ok(())
}

#[test]
fn senseless_combinations_are_usage_errors() -> TestResult {
    for args in [
        &["--flat", "--combined"][..],
        &["--flat", "--tags"],
        &["--flat", "--format", "json"],
        &["--flat", "--format", "xml"],
        &["--format", "csv"],
        &["--format", "csv-strict"],
        &["--flat", "--format", "csv-strict", "--csv-raw"],
        &["--csv-raw"],
        &["--flat", "--format", "ndjson", "--csv-raw"],
    ] {
        let (dir, text) = export(args, 2)?;
        assert!(!dir.path().join("out").exists(), "{args:?}: {text}");
    }
    Ok(())
}
