// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::io::{self, Write as _};
use std::path::Path;

use p4kx_extraction::OriginDigestWriter;
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestRecord, TestStruct, TestType, TestValue,
};
use serde_json::Value;

use super::{TestResult, export, write_archive_with_text};

const CSV: &str = "values.csv";
const CSV_COMPANION: &str = "values.csv.p4kx-origin.jsonl";
const TRANSLATION: &str = "Cart \"Co\",\\nnext";
const CSV_TEXT: &[u8] = b"\"Cart \"\"Co\"\",\nnext\"";

fn run(archive: &Path, out: &Path, args: &[&str], exit: i32) -> Result<Value, Box<dyn Error>> {
    let output = p4kx_testkit::p4kx()?
        .args(["dcb", "export"])
        .arg(archive)
        .arg("--out")
        .arg(out)
        .arg("--json")
        .args(args)
        .output()?;
    assert_eq!(output.status.code(), Some(exit), "exit code: {output:?}");
    Ok(serde_json::from_slice(&output.stdout)?)
}

fn receipt(path: &Path, bytes: &[u8]) -> Result<Vec<Value>, Box<dyn Error>> {
    let mut measured = OriginDigestWriter::new(io::sink());
    measured.write_all(bytes)?;
    let digest = serde_json::to_value(measured.finish()?)?;
    let source = fs::read_to_string(path)?;
    let rows = source
        .lines()
        .map(serde_json::from_str::<Value>)
        .collect::<Result<Vec<_>, _>>()?;
    let head = rows.first().ok_or("receipt header")?;
    assert_eq!(
        head.pointer("/binding/byteLength"),
        digest.get("byteLength"),
        "the binding measures the complete output"
    );
    assert_eq!(
        head.pointer("/binding/sha256"),
        digest.get("sha256"),
        "the binding authenticates the actual bytes"
    );
    assert_eq!(
        rows.last()
            .and_then(|row| row.get("type"))
            .and_then(Value::as_str),
        Some("end"),
        "the companion is complete"
    );
    Ok(rows)
}

fn assert_locale_edges(rows: &[Value], field: &Value) -> TestResult {
    assert_eq!(
        field.get("sourceSetComplete").and_then(Value::as_bool),
        Some(true),
        "the locale source set is complete"
    );
    let refs = field
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("locale input uses")?;
    let selected: Vec<_> = rows
        .iter()
        .filter_map(|row| row.get("sourceUse"))
        .filter(|source| source.get("id").is_some_and(|id| refs.contains(id)))
        .collect();
    assert!(
        selected.iter().any(|source| {
            source.pointer("/location/type").and_then(Value::as_str) == Some("localizationLookup")
        }),
        "the emitted text retains its indexed lookup"
    );
    assert!(
        selected.iter().any(|source| {
            source
                .pointer("/location/namespace")
                .and_then(Value::as_str)
                == Some("globalIniBytes")
        }),
        "the emitted text retains the exact original value"
    );
    Ok(())
}

fn byte_range(address: &Value) -> Result<std::ops::Range<usize>, Box<dyn Error>> {
    let number = |field| -> Result<usize, Box<dyn Error>> {
        Ok(address
            .get(field)
            .and_then(Value::as_str)
            .ok_or("decimal offset")?
            .parse()?)
    };
    Ok(number("start")?..number("endExclusive")?)
}

#[test]
fn flat_csv_dialects_bind_the_exact_multiline_published_cell() -> TestResult {
    let root = tempfile::tempdir()?;
    let archive = write_archive_with_text(root.path(), TRANSLATION)?;
    for mode in [
        &["--flat", "--format", "csv"][..],
        &["--flat", "--format", "csv", "--csv-raw"],
        &["--flat", "--format", "csv-strict"],
    ] {
        let out = root.path().join(format!("out-{}", mode.join("_")));
        let mut args = mode.to_vec();
        args.extend(["--lang", "en"]);
        let report = run(&archive, &out, &args, 0)?;
        assert_eq!(
            report
                .get("origin_receipts")
                .and_then(Value::as_array)
                .map(Vec::len),
            Some(1),
            "{mode:?} writes one companion"
        );
        let bytes = fs::read(out.join(CSV))?;
        let rows = receipt(&out.join(CSV_COMPANION), &bytes)?;
        let text = rows
            .iter()
            .find(|row| {
                row.pointer("/address/type").and_then(Value::as_str) == Some("textBytes")
                    && row.pointer("/address/field").and_then(Value::as_str) == Some("/rows/0/text")
            })
            .ok_or("first data row translation")?;
        let address = text.get("address").ok_or("CSV address")?;
        assert_eq!(
            bytes.get(byte_range(address)?),
            Some(CSV_TEXT),
            "{mode:?} quotes the emitted cell in its measured range"
        );
        assert_locale_edges(&rows, text)?;
    }
    Ok(())
}

#[test]
fn compact_and_flat_ndjson_bind_the_indexed_locale_to_actual_output_lines() -> TestResult {
    let root = tempfile::tempdir()?;
    let archive = write_archive_with_text(root.path(), TRANSLATION)?;
    for (file, path, flat) in [
        ("records.ndjson", "/name/text", false),
        ("values.ndjson", "/text", true),
    ] {
        let out = root.path().join(file);
        let mut args = vec!["--format", "ndjson", "--lang", "en"];
        if flat {
            args.push("--flat");
        }
        run(&archive, &out, &args, 0)?;
        let bytes = fs::read(out.join(file))?;
        let rows = receipt(&out.join(format!("{file}.p4kx-origin.jsonl")), &bytes)?;
        let text = rows
            .iter()
            .find(|row| {
                row.pointer("/address/line").and_then(Value::as_str) == Some("0")
                    && row.pointer("/address/path").and_then(Value::as_str) == Some(path)
            })
            .ok_or("first NDJSON line translation")?;
        let first = str::from_utf8(&bytes)?
            .lines()
            .next()
            .ok_or("payload line")?;
        let payload: Value = serde_json::from_str(first)?;
        assert_eq!(
            payload.pointer(path).and_then(Value::as_str),
            Some("Cart \"Co\",\nnext"),
            "{file} keeps the decoded translation"
        );
        assert_locale_edges(&rows, text)?;
    }
    Ok(())
}

#[test]
fn a_strict_csv_refusal_keeps_the_previous_file_and_companion() -> TestResult {
    let (root, _) = export(&["--flat", "--format", "csv-strict", "--lang", "en"], 0)?;
    let out = root.path().join("out");
    let payload = fs::read(out.join(CSV))?;
    let proof = fs::read(out.join(CSV_COMPANION))?;
    let invalid = root.path().join("invalid");
    fs::create_dir(&invalid)?;
    let invalid = write_archive_with_text(&invalid, "Cräte")?;

    let report = run(
        &invalid,
        &out,
        &[
            "--flat",
            "--format",
            "csv-strict",
            "--lang",
            "en",
            "--conflict",
            "overwrite",
        ],
        1,
    )?;
    assert!(
        report
            .get("error")
            .and_then(Value::as_str)
            .is_some_and(|error| error.contains("RFC 4180")),
        "strict CSV reports its encoding refusal"
    );
    assert_eq!(
        report.get("exported").and_then(Value::as_u64),
        Some(0),
        "no invalid Record file commits"
    );
    assert_eq!(
        fs::read(out.join(CSV))?,
        payload,
        "keeps the earlier CSV bytes"
    );
    assert_eq!(
        fs::read(out.join(CSV_COMPANION))?,
        proof,
        "keeps the earlier bound proof"
    );
    assert_eq!(
        fs::read_dir(&out)?.count(),
        2,
        "uncommitted temporary files are retired"
    );
    Ok(())
}

#[test]
fn flat_csv_rebases_cells_after_the_first_worker_part() -> TestResult {
    const COUNT: u32 = 65;
    let directory = tempfile::tempdir()?;
    let database = (0..COUNT).fold(
        DataForgeBuilder::new().structure(TestStruct::new("Item").field("n", TestType::UInt32)),
        |builder, n| {
            builder.record(
                TestRecord::new(
                    u128::from(n) + 1,
                    "Item",
                    format!("Item.{n}"),
                    format!("items/{n:05}.xml"),
                )
                .value("n", TestValue::UInt32(1000 + n)),
            )
        },
    );
    let archive = directory.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data\\Game2.dcb", database.build()))
        .write_to(&archive)?;
    let out = directory.path().join("out");
    run(
        &archive,
        &out,
        &["--flat", "--format", "csv", "--csv-raw"],
        0,
    )?;
    let bytes = fs::read(out.join(CSV))?;
    let rows = receipt(&out.join(CSV_COMPANION), &bytes)?;
    for (path, value) in [("/rows/63/value", b"1063"), ("/rows/64/value", b"1064")] {
        let address = rows
            .iter()
            .filter_map(|row| row.get("address"))
            .find(|address| address.get("field").and_then(Value::as_str) == Some(path))
            .ok_or("specific data row position")?;
        assert_eq!(
            bytes.get(byte_range(address)?),
            Some(value.as_slice()),
            "{path}"
        );
    }
    Ok(())
}
