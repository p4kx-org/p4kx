// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! `p4kx dcb export --lang` end to end: resolved texts next to the localisation keys of the
//! Records, over a synthetic DataForge and synthetic `global.ini` files.

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, GlobalIniBuilder, TestEntry, TestRecord, TestStruct,
    TestType, TestValue,
};
use serde_json::Value;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const COMPANION_SUFFIX: &str = ".p4kx-origin.jsonl";

const CART: u128 = 0xcbee_34fd_6441_4000_b99f_924f_7f26_e19f;
const TRUCK: u128 = 0x50e8_a9f6_f956_4000_8ace_7c0c_a9a9_1f5d;

fn database() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .structure(
            TestStruct::new("Vehicle")
                .field("name", TestType::Locale)
                .field("description", TestType::Locale)
                .field("unset", TestType::Locale)
                .array("labels", TestType::Locale),
        )
        .record(
            TestRecord::new(
                CART,
                "Vehicle",
                "Vehicle.Cart",
                "libs/foundry/records/vehicles/cart.xml",
            )
            .value("name", TestValue::text("@vehicle_Cart"))
            .value("description", TestValue::text("@vehicle_CartDesc"))
            .value("unset", TestValue::text("@LOC_PLACEHOLDER"))
            .value(
                "labels",
                TestValue::Array(vec![
                    TestValue::text("@vehicle_Cart"),
                    TestValue::text("@vehicle_Unknown"),
                ]),
            ),
        )
        .record(
            TestRecord::new(
                TRUCK,
                "Vehicle",
                "Vehicle.Truck",
                "libs/foundry/records/vehicles/truck.xml",
            )
            .value("name", TestValue::text("@vehicle_Truck"))
            .value("description", TestValue::text("@vehicle_TruckDesc"))
            .value("unset", TestValue::text(""))
            .value("labels", TestValue::Array(Vec::new())),
        )
}

fn write_archive(dir: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let path = dir.join("Data.p4k");
    let english = GlobalIniBuilder::new()
        .text("vehicle_Cart", "Cart & \"Co\"")
        .text("vehicle_CartDesc", "Rolls.\\nSlowly.")
        .text("vehicle_Truck", "Truck")
        .text("vehicle_TruckDesc", "Hauls.")
        .build();
    let german = GlobalIniBuilder::new()
        .text("vehicle_Cart", "Karren")
        .text("vehicle_CartDesc", "")
        .text("vehicle_Truck", "Laster")
        .build();
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data\\Game2.dcb", database().build()))
        .entry(TestEntry::zstd(
            "Data\\Localization\\english\\global.ini",
            english,
        ))
        .entry(TestEntry::zstd(
            "Data\\Localization\\german_(germany)\\global.ini",
            german,
        ))
        .write_to(&path)?;
    Ok(path)
}

fn export(archive: &Path, out: &Path, args: &[&str], code: i32) -> Result<String, Box<dyn Error>> {
    let output = p4kx_testkit::p4kx()?
        .args(["dcb", "export"])
        .arg(archive)
        .arg("--out")
        .arg(out)
        .args(args)
        .output()?;
    assert_eq!(output.status.code(), Some(code), "exit code: {output:?}");
    Ok(String::from_utf8(output.stdout)?)
}

fn tree(root: &Path) -> Result<String, Box<dyn Error>> {
    let mut files = Vec::new();
    let mut folders = vec![root.to_path_buf()];
    while let Some(folder) = folders.pop() {
        for entry in fs::read_dir(folder)? {
            let path = entry?.path();
            if path.is_dir() {
                folders.push(path);
            } else if !path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(COMPANION_SUFFIX))
            {
                files.push(path);
            }
        }
    }
    files.sort();
    let mut listing = String::new();
    for file in files {
        let relative = file.strip_prefix(root)?.display().to_string();
        listing.push_str("== ");
        listing.push_str(&relative.replace('\\', "/"));
        listing.push('\n');
        listing.push_str(&fs::read_to_string(&file)?);
    }
    Ok(listing)
}

fn exported(args: &[&str]) -> TestResult<String> {
    with_exported(args, tree)
}

fn with_exported<T>(args: &[&str], inspect: impl FnOnce(&Path) -> TestResult<T>) -> TestResult<T> {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;
    let out = dir.path().join("out");
    export(&archive, &out, args, 0)?;
    inspect(&out)
}

#[test]
fn xml_with_a_language_carries_the_text_or_the_missing_mark() -> TestResult {
    insta::assert_snapshot!("xml_english", exported(&["--lang", "en"])?);
    insta::assert_snapshot!("xml_german", exported(&["--lang", "de"])?);
    Ok(())
}

#[test]
fn json_and_ndjson_with_a_language_carry_the_text_or_the_missing_mark() -> TestResult {
    insta::assert_snapshot!(
        "json_english",
        exported(&["--format", "json", "--lang", "en"])?
    );
    let ndjson = exported(&["--format", "ndjson", "--lang", "de"])?;
    insta::assert_snapshot!("ndjson_german", ndjson);
    for line in ndjson.lines().skip(1) {
        serde_json::from_str::<Value>(line)?;
    }
    Ok(())
}

#[test]
fn the_combined_document_declares_the_namespace_once_and_resolves_texts() -> TestResult {
    let text = exported(&["--combined", "--lang", "en"])?;

    insta::assert_snapshot!("combined_english", text);
    roxmltree::Document::parse(text.split_once('\n').map_or("", |(_, rest)| rest))?;
    Ok(())
}

#[test]
fn without_a_language_the_output_has_no_texts_and_no_marks() -> TestResult {
    for args in [
        &["--format", "xml"][..],
        &["--format", "json"],
        &["--format", "ndjson"],
        &["--combined"],
    ] {
        with_exported(args, |out| {
            let text = tree(out)?;
            assert!(
                !text.contains("p4kx:text")
                    && !text.contains("missing")
                    && !text.contains("\"text\""),
                "{args:?}: {text}"
            );
            assert!(
                text.contains("\"@vehicle_Cart\""),
                "{args:?}: the original key stays"
            );
            if args == ["--format", "json"] {
                assert_only_dataforge_sources(out)?;
            }
            Ok(())
        })?;
    }
    Ok(())
}

fn assert_only_dataforge_sources(out: &Path) -> TestResult {
    let records = out.join("libs/foundry/records/vehicles");
    for name in ["cart", "truck"] {
        let path = records.join(format!("{name}.json{COMPANION_SUFFIX}"));
        let rows = fs::read_to_string(path)?
            .lines()
            .map(serde_json::from_str::<Value>)
            .collect::<Result<Vec<_>, _>>()?;
        let entries: Vec<_> = rows
            .iter()
            .filter(|row| {
                matches!(
                    row.pointer("/source/kind").and_then(Value::as_str),
                    Some("archiveEntry" | "archiveEntryProof")
                )
            })
            .map(|row| row.pointer("/source/path/value").and_then(Value::as_str))
            .collect();
        assert_eq!(
            entries,
            [Some(r"Data\Game2.dcb")],
            "{name}: only the original DataForge Entry contributes"
        );
    }
    Ok(())
}

#[test]
fn output_with_a_language_is_byte_identical_across_runs() -> TestResult {
    for args in [
        &["--lang", "de"][..],
        &["--format", "json", "--lang", "de"],
        &["--format", "ndjson", "--lang", "de"],
        &["--combined", "--lang", "de"],
    ] {
        assert_eq!(exported(args)?, exported(args)?, "{args:?}");
    }
    Ok(())
}

#[test]
fn the_report_names_the_language_only_when_one_is_chosen() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;
    let run = |out: &str, extra: &[&str]| -> Result<Value, Box<dyn Error>> {
        let output = p4kx_testkit::p4kx()?
            .arg("--json")
            .args(["dcb", "export"])
            .arg(&archive)
            .arg("--out")
            .arg(dir.path().join(out))
            .args(extra)
            .output()?;
        assert_eq!(output.status.code(), Some(0), "{output:?}");
        Ok(serde_json::from_slice(&output.stdout)?)
    };

    let with = run("with", &["--lang", "de"])?;
    let without = run("without", &[])?;

    assert_eq!(with["language"], "de");
    assert!(without.get("language").is_none(), "{without}");
    Ok(())
}

#[test]
fn a_language_the_archive_holds_no_file_for_is_an_error() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;
    let out = dir.path().join("out");

    let output = p4kx_testkit::p4kx()?
        .args(["dcb", "export"])
        .arg(&archive)
        .arg("--out")
        .arg(&out)
        .args(["--lang", "ja"])
        .output()?;

    assert_ne!(output.status.code(), Some(0), "{output:?}");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("no global.ini for the language"),
        "{output:?}"
    );
    assert!(!out.exists(), "nothing is written");
    Ok(())
}
