// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};

use p4kx_extraction::OriginDigestWriter;
use p4kx_testkit::{ArchiveBuilder, TestChunkFile, TestEntry, test_key};
use serde_json::{Value, json};

use super::{MODEL_ENTRY, TestResult, ivo_model, p4kx, soc_file};

const SOC_ENTRY: &str = r"Data\Objects\Test\room.soc";

#[test]
fn an_encrypted_xml_dump_addresses_original_fields_in_the_selected_chunk() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = dir.path().join("Data.p4k");
    let original = soc_file();
    let position = le_word(&original, 44..48)?;
    ArchiveBuilder::new()
        .entry(TestEntry::zstd(SOC_ENTRY, original).encrypted())
        .write_to(&archive)?;
    let out = dir.path().join("converted.xml");
    let key = test_key().to_hex();

    let output = p4kx()?
        .args(["--key", &key, "--json", "chunks", "dump"])
        .arg(&archive)
        .arg(SOC_ENTRY)
        .args(["--chunk", "1", "--text", "--out"])
        .arg(&out)
        .output()?;

    assert_eq!(output.status.code(), Some(0), "success: {output:?}");
    let report: Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(report["chunk"]["offset"], position, "selected original row");
    assert_eq!(report["originReceipt"], "converted.xml.p4kx-origin.jsonl");
    assert_eq!(
        report["fieldOrigins"]["/file"]["containsEncryptedSource"],
        false
    );
    assert_eq!(
        report["fieldOrigins"]["/chunk/type_id"]["containsEncryptedSource"],
        true
    );
    check_row_origin(&report)?;
    check_converted_xml(&out, position)
}

fn check_row_origin(report: &Value) -> TestResult {
    let refs = report
        .pointer("/fieldOrigins/~1chunk~1type_id/sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("selected chunk field references")?;
    let uses = report
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("table uses")?;
    assert!(
        uses.iter().any(|usage| {
            usage.get("id").is_some_and(|id| refs.contains(id))
                && usage.pointer("/location/indices") == Some(&json!(["1", "32", "34"]))
                && usage.pointer("/location/field").and_then(Value::as_str) == Some("type")
        }),
        "the type ID uses the physical column of row one"
    );
    Ok(())
}

fn check_converted_xml(out: &Path, position: u32) -> TestResult {
    let bytes = fs::read(out)?;
    let rows = receipt(out)?;
    let field = rows
        .iter()
        .find(|row| {
            row.pointer("/address/type").and_then(Value::as_str) == Some("xmlBytes")
                && row.pointer("/address/field").and_then(Value::as_str) == Some("elementName")
        })
        .ok_or("addressed root XML name")?;
    let start = decimal(field, "/address/start")?;
    let end = decimal(field, "/address/endExclusive")?;
    assert_eq!(
        bytes.get(start..end),
        Some(b"Entities".as_slice()),
        "observed output token"
    );
    assert_eq!(
        field.get("containsEncryptedSource"),
        Some(&Value::Bool(true)),
        "used encrypted payload"
    );
    let refs = field
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("XML field refs")?;
    assert!(
        rows.iter()
            .filter_map(|row| row.get("sourceUse"))
            .any(|usage| {
                usage.get("id").is_some_and(|id| refs.contains(id))
                    && usage.pointer("/location/namespace").and_then(Value::as_str)
                        == Some("embeddedCryXmlNode")
                    && usage.pointer("/location/indices")
                        == Some(&json!([position.to_string(), "0"]))
            }),
        "original decoded file offset and original CryXml node ordinal"
    );
    assert!(
        rows.iter()
            .filter_map(|row| row.get("sourceUse"))
            .any(|usage| {
                usage.get("id").is_some_and(|id| refs.contains(id))
                    && usage.get("role").and_then(Value::as_str) == Some("selector")
                    && usage.pointer("/location/field").and_then(Value::as_str) == Some("offset")
                    && usage.pointer("/location/indices") == Some(&json!(["1", "44", "48"]))
            }),
        "the emitted XML field carries the actual common table selector"
    );
    Ok(())
}

#[test]
fn encrypted_raw_dumps_keep_the_selected_decoded_range() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = dir.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::zstd(MODEL_ENTRY, ivo_model()).encrypted())
        .write_to(&archive)?;
    let key = test_key().to_hex();
    let out = dir.path().join("mesh.bin");
    p4kx()?
        .args(["--key", &key, "chunks", "dump"])
        .arg(&archive)
        .arg(MODEL_ENTRY)
        .args(["--chunk", "2", "--out"])
        .arg(&out)
        .assert()
        .code(0);

    assert_eq!(
        fs::read(&out)?,
        [9; 40],
        "the selected payload is untouched"
    );
    let rows = receipt(&out)?;
    let payload = rows
        .iter()
        .find(|row| row.pointer("/address/type").and_then(Value::as_str) == Some("opaquePayload"))
        .ok_or("opaque payload")?;
    assert_eq!(
        payload.get("containsEncryptedSource"),
        Some(&Value::Bool(true))
    );
    let refs = payload
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("payload references")?;
    assert!(
        rows.iter()
            .filter_map(|row| row.get("sourceUse"))
            .any(|usage| {
                usage.get("id").is_some_and(|id| refs.contains(id))
                    && usage.pointer("/location/namespace").and_then(Value::as_str)
                        == Some("cryEngineChunkPayload")
                    && usage.pointer("/location/indices") == Some(&json!(["2", "112", "152"]))
            }),
        "the source is the original decoded chunk and its exact half-open byte range"
    );
    Ok(())
}

#[test]
fn standalone_dump_receipts_keep_utf8_and_empty_payloads_opaque() -> TestResult {
    let dir = tempfile::tempdir()?;
    let input = dir.path().join("input.soc");
    fs::write(
        &input,
        TestChunkFile::crch()
            .chunk(0x0011, 1, *b"{}")
            .chunk(0x1234, 1, [])
            .bytes(),
    )?;
    for (index, extra, bytes) in [
        ("0", &["--text"][..], b"{}".as_slice()),
        ("1", &[][..], b"".as_slice()),
    ] {
        let out = dir.path().join(format!("chunk-{index}.bin"));
        p4kx()?
            .args(["chunks", "dump"])
            .arg(&input)
            .args(["--chunk", index, "--out"])
            .arg(&out)
            .args(extra)
            .assert()
            .code(0);
        assert_eq!(fs::read(&out)?, bytes, "the dump is byte-identical");
        let rows = receipt(&out)?;
        let payload = rows
            .iter()
            .find(|row| {
                row.pointer("/address/type").and_then(Value::as_str) == Some("opaquePayload")
            })
            .ok_or("opaque bytes")?;
        assert_eq!(
            payload.get("containsEncryptedSource"),
            Some(&Value::Null),
            "unknown original lineage"
        );
        assert_eq!(
            payload.get("sourceSetComplete"),
            Some(&Value::Bool(true)),
            "physical input observed"
        );
    }
    Ok(())
}

#[test]
fn a_failed_text_conversion_preserves_the_previous_artifact_and_companion() -> TestResult {
    let dir = tempfile::tempdir()?;
    let input = dir.path().join("room.soc");
    fs::write(&input, soc_file())?;
    let out = dir.path().join("out.xml");
    p4kx()?
        .args(["chunks", "dump"])
        .arg(&input)
        .args(["--text", "--chunk", "1", "--out"])
        .arg(&out)
        .assert()
        .code(0);
    let previous = fs::read(&out)?;
    let prior_receipt = fs::read(companion(&out)?)?;

    p4kx()?
        .args(["chunks", "dump"])
        .arg(&input)
        .args(["--text", "--chunk", "5", "--out"])
        .arg(&out)
        .assert()
        .code(3);

    assert_eq!(fs::read(&out)?, previous);
    assert_eq!(fs::read(companion(&out)?)?, prior_receipt);
    Ok(())
}

#[test]
fn a_companion_hard_link_cannot_overwrite_the_original_file() -> TestResult {
    let dir = tempfile::tempdir()?;
    let input = dir.path().join("room.soc");
    let original = soc_file();
    fs::write(&input, &original)?;
    let out = dir.path().join("out.bin");
    let sidecar = companion(&out)?;
    fs::hard_link(&input, &sidecar)?;

    p4kx()?
        .args(["chunks", "dump"])
        .arg(&input)
        .args(["--chunk", "1", "--out"])
        .arg(&out)
        .assert()
        .code(2);

    assert!(!out.exists());
    assert_eq!(fs::read(&input)?, original);
    assert_eq!(fs::read(sidecar)?, original);
    Ok(())
}

fn le_word(input: &[u8], range: std::ops::Range<usize>) -> Result<u32, Box<dyn Error>> {
    Ok(u32::from_le_bytes(
        input.get(range).ok_or("fixture word")?.try_into()?,
    ))
}

fn decimal(input: &Value, path: &str) -> Result<usize, Box<dyn Error>> {
    Ok(input
        .pointer(path)
        .and_then(Value::as_str)
        .ok_or("decimal byte offset")?
        .parse()?)
}

fn companion(out: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let mut name = out.file_name().ok_or("output basename")?.to_os_string();
    name.push(".p4kx-origin.jsonl");
    Ok(out.with_file_name(name))
}

fn receipt(out: &Path) -> Result<Vec<Value>, Box<dyn Error>> {
    let file = fs::read_to_string(companion(out)?)?;
    let rows = file
        .lines()
        .map(serde_json::from_str::<Value>)
        .collect::<Result<Vec<_>, _>>()?;
    let binding = rows
        .first()
        .and_then(|row| row.get("binding"))
        .ok_or("file binding")?;
    assert_eq!(
        binding.pointer("/path/value").and_then(Value::as_str),
        out.file_name().and_then(std::ffi::OsStr::to_str),
        "the companion binds the output basename"
    );
    let mut measured = OriginDigestWriter::new(io::sink());
    measured.write_all(&fs::read(out)?)?;
    let digest = serde_json::to_value(measured.finish()?)?;
    for field in ["byteLength", "sha256"] {
        assert_eq!(
            binding.get(field),
            digest.get(field),
            "binding {field} measures the dump"
        );
    }
    assert_eq!(
        rows.last()
            .and_then(|row| row.get("type"))
            .and_then(Value::as_str),
        Some("end"),
        "completed receipt"
    );
    Ok(rows)
}
