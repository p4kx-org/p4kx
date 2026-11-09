// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! `p4kx catalog` end to end, over a synthetic DataForge and English and German `global.ini`
//! files.

use crate::catalog_support;

use std::error::Error;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use assert_cmd::Command;
use catalog_support::{EXPORTED_AT, english, write_archive, write_archive_with};
use p4kx_testkit::TestEntry;

type TestResult = Result<(), Box<dyn Error>>;

fn p4kx(args: &[&str]) -> Result<Command, Box<dyn Error>> {
    let mut command = p4kx_testkit::p4kx()?;
    command.args(args).env("SOURCE_DATE_EPOCH", EXPORTED_AT);
    Ok(command)
}

fn catalog(archive: &Path, out: &Path, args: &[&str]) -> Result<String, Box<dyn Error>> {
    let output = p4kx(&["catalog"])?
        .arg(archive)
        .arg("--out")
        .arg(out)
        .args(args)
        .output()?;
    assert_eq!(output.status.code(), Some(0), "exit code: {output:?}");
    let dir = archive
        .parent()
        .and_then(Path::parent)
        .ok_or("the Archive has a folder")?
        .display()
        .to_string();
    Ok(String::from_utf8(output.stdout)?
        .replace(&dir.replace('\\', "\\\\"), "[DIR]")
        .replace(&dir, "[DIR]")
        .replace("\\\\", "/")
        .replace('\\', "/"))
}

fn printed_schema() -> Result<String, Box<dyn Error>> {
    let output = p4kx(&["catalog", "schema"])?.output()?;
    assert_eq!(output.status.code(), Some(0), "exit code: {output:?}");
    Ok(String::from_utf8(output.stdout)?)
}

fn redact_version(text: &str) -> String {
    text.replace(
        &format!("\"toolVersion\":\"{}\"", env!("CARGO_PKG_VERSION")),
        "\"toolVersion\":\"[VERSION]\"",
    )
}

fn manufacturer_name(
    catalog: &serde_json::Value,
    index: usize,
) -> Result<&serde_json::Value, Box<dyn Error>> {
    let row = catalog
        .pointer("/definitions/manufacturers/rows")
        .and_then(|rows| rows.get(index))
        .ok_or("manufacturer")?;
    assert_eq!(
        row.pointer("/localization/state")
            .and_then(serde_json::Value::as_str),
        Some("present"),
        "the selected Manufacturer has an authored localization"
    );
    let reference = row
        .pointer("/localization/value/name")
        .ok_or("name carrier")?;
    assert_eq!(
        reference.get("state").and_then(serde_json::Value::as_str),
        Some("present"),
        "the Manufacturer name has a canonical reference"
    );
    let target = reference
        .get("value")
        .and_then(serde_json::Value::as_str)
        .ok_or("name reference")?;
    catalog
        .pointer("/texts/publicNames")
        .and_then(serde_json::Value::as_array)
        .ok_or("public names")?
        .iter()
        .find(|text| {
            text.pointer("/id/value")
                .and_then(serde_json::Value::as_str)
                == Some(target)
        })
        .ok_or_else(|| "canonical manufacturer name".into())
}

#[test]
fn catalog_schema_prints_the_checked_in_schema() -> TestResult {
    let checked_in = fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../schemas/basetool-catalog.schema.json"),
    )?;
    assert!(
        printed_schema()? == checked_in,
        "the printed schema is the checked-in one"
    );
    Ok(())
}

#[test]
fn catalog_writes_a_catalog_that_the_schema_accepts() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;
    let out = dir.path().join("basetool-catalog.json");

    let summary = catalog(&archive, &out, &[])?;

    let text = fs::read_to_string(&out)?;
    let schema: serde_json::Value = serde_json::from_str(&printed_schema()?)?;
    let validator = jsonschema::validator_for(&schema)?;
    let instance: serde_json::Value = serde_json::from_str(&text)?;
    let errors: Vec<String> = validator
        .iter_errors(&instance)
        .map(|error| error.to_string())
        .collect();
    assert!(errors.is_empty(), "valid against the schema: {errors:?}");
    assert!(instance.get("manufacturers").is_none());
    assert_eq!(
        instance["definitions"]["manufacturers"]["availability"],
        "available"
    );
    assert_eq!(
        manufacturer_name(&instance, 1)?["key"]["value"],
        "@manufacturer_NameACAS"
    );
    insta::assert_snapshot!("catalog_file", redact_version(&text));
    insta::assert_snapshot!("catalog_summary", summary);
    Ok(())
}

#[test]
fn catalog_to_a_zst_name_writes_a_zstd_frame_with_the_bytes_of_the_plain_catalog() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;
    let plain = dir.path().join("plain.json");
    let packed = dir.path().join("basetool-catalog.json.zst");

    catalog(&archive, &plain, &[])?;
    let summary = catalog(&archive, &packed, &["--json"])?;

    let frame = fs::read(&packed)?;
    assert_eq!(frame.get(..4), Some(&[0x28, 0xb5, 0x2f, 0xfd][..]), "magic");
    let mut decoded = Vec::new();
    zstd::Decoder::new(frame.as_slice())?.read_to_end(&mut decoded)?;
    assert!(decoded == fs::read(&plain)?, "byte-identical JSON");
    let json: serde_json::Value = serde_json::from_str(&summary)?;
    assert_eq!(json["size"], decoded.len(), "uncompressed size");
    assert_eq!(json["compressed_size"], frame.len(), "compressed size");
    assert_eq!(json["decompressed_limit"], 1_000_000_000, "limit");
    Ok(())
}

#[test]
fn catalog_to_a_json_name_stays_plain_json_without_a_compressed_size() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;
    let out = dir.path().join("plain.json");

    let summary = catalog(&archive, &out, &["--json"])?;

    assert_eq!(fs::read(&out)?.first(), Some(&b'{'), "plain JSON");
    let json: serde_json::Value = serde_json::from_str(&summary)?;
    assert_eq!(json["compressed_size"], serde_json::Value::Null, "none");
    Ok(())
}

#[test]
fn catalog_source_carries_the_public_patch_the_launcher_log_names() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;
    let logs = dir.path().join("logs");
    fs::create_dir_all(&logs)?;
    fs::write(
        logs.join("log.log"),
        "Delta update applied (SC LIVE 4.4.1-live.10012345) in C:\\SC\\LIVE\n",
    )?;
    let out = dir.path().join("with-patch.json");
    let output = p4kx(&["catalog"])?
        .env("P4KX_LAUNCHER_LOGS", &logs)
        .arg(&archive)
        .arg("--out")
        .arg(&out)
        .output()?;
    assert_eq!(output.status.code(), Some(0), "exit code: {output:?}");

    let catalog: serde_json::Value = serde_json::from_str(&fs::read_to_string(&out)?)?;
    let schema: serde_json::Value = serde_json::from_str(&printed_schema()?)?;
    assert!(
        jsonschema::validator_for(&schema)?.is_valid(&catalog),
        "valid against the schema"
    );
    assert_eq!(catalog["source"]["build"]["publicPatch"], "4.4.1");
    Ok(())
}

#[test]
fn catalog_with_a_filter_marks_the_kind_partial_and_prints_json() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;
    let out = dir.path().join("partial.json");

    let summary = catalog(&archive, &out, &["--filter", "*/acas", "--json"])?;

    let catalog: serde_json::Value = serde_json::from_str(&fs::read_to_string(&out)?)?;
    assert_eq!(
        catalog["completeness"]["manufacturers"],
        serde_json::json!({"status": "partial", "filter": "*/acas"}),
        "partial with the Filter"
    );
    assert_eq!(
        catalog.get("schemaVersion"),
        Some(&serde_json::json!(3)),
        "filtered output uses the current catalog version"
    );
    assert!(
        catalog.get("commodities").is_none() && catalog.get("manufacturers").is_none(),
        "filtered output has no legacy commodity array"
    );
    for pointer in [
        "/definitions/commodities/rows",
        "/projections/commodityPresentation/rows",
    ] {
        assert_eq!(
            catalog.pointer(pointer),
            Some(&serde_json::json!([])),
            "the manufacturer filter excludes commodity rows at {pointer}"
        );
    }
    insta::assert_snapshot!("catalog_filter_json", summary);
    Ok(())
}

#[test]
fn two_catalog_runs_write_identical_bytes() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;
    let first = dir.path().join("first.json");
    let second = dir.path().join("second.json");
    catalog(&archive, &first, &[])?;
    catalog(&archive, &second, &[])?;
    assert!(fs::read(first)? == fs::read(second)?, "byte-identical");
    Ok(())
}

#[test]
fn catalog_without_an_output_file_is_invalid_input() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;
    let output = p4kx(&["catalog"])?.arg(&archive).output()?;
    assert_eq!(output.status.code(), Some(2), "exit code: {output:?}");
    Ok(())
}

#[test]
fn catalog_to_a_gz_name_is_refused_instead_of_writing_plain_json() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;
    let out = dir.path().join("basetool-catalog.json.gz");

    let output = p4kx(&["catalog"])?
        .arg(&archive)
        .arg("--out")
        .arg(&out)
        .output()?;

    assert_eq!(output.status.code(), Some(2), "invalid input: {output:?}");
    let stderr = String::from_utf8(output.stderr)?;
    assert!(
        stderr.contains("gzip") && stderr.contains(".json.zst"),
        "names the cause and the alternative: {stderr}"
    );
    assert!(!out.exists(), "nothing is written");
    Ok(())
}

#[test]
fn catalog_without_a_german_global_ini_leaves_german_null_and_counts_it() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive_with(dir.path(), &[english()])?;
    let out = dir.path().join("english-only.json");

    let summary = catalog(&archive, &out, &[])?;

    let catalog: serde_json::Value = serde_json::from_str(&fs::read_to_string(&out)?)?;
    let name = manufacturer_name(&catalog, 1)?;
    assert_eq!(name["key"]["value"], "@manufacturer_NameACAS");
    assert_eq!(
        name["translations"]["value"]["en"]["value"],
        "Ace Astrogation"
    );
    assert!(
        name["translations"]["value"].get("de").is_none(),
        "no German input"
    );
    insta::assert_snapshot!("catalog_summary_without_german", summary);
    Ok(())
}

#[test]
fn catalog_reads_a_german_global_ini_with_invalid_bytes_lossily() -> TestResult {
    let dir = tempfile::tempdir()?;
    let damaged = TestEntry::zstd(
        "Data\\Localization\\german_(germany)\\global.ini",
        b"manufacturer_NameACAS=Ace\xff\r\nstray line\r\n".to_vec(),
    );
    let archive = write_archive_with(dir.path(), &[english(), damaged])?;
    let out = dir.path().join("damaged.json");
    let output = p4kx(&["catalog"])?
        .arg(&archive)
        .arg("--out")
        .arg(&out)
        .output()?;
    assert_eq!(output.status.code(), Some(0), "exit code: {output:?}");
    let stderr = String::from_utf8(output.stderr)?;
    assert!(
        stderr.contains(
            "note: Data\\Localization\\german_(germany)\\global.ini: invalid bytes replaced, 1 line skipped\n"
        ),
        "the Entry is named on stderr: {stderr}"
    );
    assert!(
        !stderr.contains("english"),
        "the clean Entry is not named: {stderr}"
    );
    let catalog: serde_json::Value = serde_json::from_str(&fs::read_to_string(out)?)?;
    assert_eq!(
        manufacturer_name(&catalog, 1)?["translations"]["value"]["de"]["value"],
        "Ace\u{fffd}",
        "the bad byte is replaced, the text kept"
    );
    Ok(())
}
