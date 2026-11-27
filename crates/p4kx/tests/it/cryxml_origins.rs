// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::io::{self, Write as _};

use p4kx_extraction::OriginDigestWriter;
use p4kx_testkit::{CryXmlByteOrder, CryXmlSignature, TestElement};
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn Error>>;

fn rows(bytes: &[u8]) -> Result<Vec<Value>, serde_json::Error> {
    bytes
        .split(|&byte| byte == b'\n')
        .filter(|line| !line.is_empty())
        .map(serde_json::from_slice)
        .collect()
}

fn measurement(bytes: &[u8]) -> Result<Value, Box<dyn Error>> {
    let mut digest = OriginDigestWriter::new(io::sink());
    digest.write_all(bytes)?;
    Ok(serde_json::to_value(digest.finish()?)?)
}

#[test]
fn binary_fields_have_original_coordinates_and_exact_emitted_byte_ranges() -> TestResult {
    let dir = tempfile::tempdir()?;
    let source = dir.path().join("source.xml");
    let output = dir.path().join("output.xml");
    let fixture = TestElement::new("item")
        .attribute("repeat", "first")
        .attribute("repeat", "<&");
    fs::write(
        &source,
        fixture.to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little),
    )?;
    let command = p4kx_testkit::p4kx()?
        .args(["--json", "cryxml"])
        .arg(&source)
        .arg("--out")
        .arg(&output)
        .output()?;
    assert!(command.status.success(), "{command:?}");
    assert!(command.stderr.is_empty(), "{command:?}");
    let summary: Value = serde_json::from_slice(&command.stdout)?;
    assert_eq!(summary["format"], "CryXmlB");
    assert_eq!(summary["byte_order"], "little");
    assert_eq!(summary["originReceipt"], "output.xml.p4kx-origin.jsonl");
    assert_eq!(
        summary["fieldOrigins"]["/input"]["containsEncryptedSource"],
        Value::Null
    );
    assert_eq!(
        summary["fieldOrigins"]["/format"]["containsEncryptedSource"],
        Value::Null
    );
    assert_eq!(
        summary["fieldOrigins"]["/byte_order"]["containsEncryptedSource"],
        Value::Null
    );
    assert_eq!(
        summary["fieldOrigins"]["/output"]["containsEncryptedSource"],
        false
    );

    let payload = fs::read(output)?;
    let receipt = rows(&fs::read(dir.path().join("output.xml.p4kx-origin.jsonl"))?)?;
    let digest = measurement(&payload)?;
    assert_eq!(
        receipt.first().ok_or("missing header")?,
        &json!({
            "type": "header", "format": "p4kx-source-origin", "formatVersion": "1.0",
            "binding": {"type": "file", "path": {"encoding": "utf8", "value": "output.xml"},
                "sha256": digest["sha256"], "byteLength": digest["byteLength"]}
        })
    );
    check_binary_addresses(&receipt, &payload)
}

fn check_binary_addresses(receipt: &[Value], payload: &[u8]) -> TestResult {
    let suffix = receipt
        .iter()
        .filter(|row| {
            row.pointer("/address/field").and_then(Value::as_str) == Some("attributeName")
        })
        .find(|row| range(row, payload) == Some(b"repeat_2"))
        .ok_or("missing suffix")?;
    assert_eq!(
        suffix["containsEncryptedSource"],
        Value::Null,
        "the standalone source has unknown encryption"
    );
    let uses = suffix["sourceUsesRefs"]
        .as_array()
        .ok_or("missing field source uses")?;
    assert_eq!(
        uses.len(),
        2,
        "the suffixed name has a value and a selection input"
    );
    let addresses: Vec<_> = receipt
        .iter()
        .filter_map(|row| row.get("sourceUse"))
        .filter(|source_use| source_use.get("id").is_some_and(|id| uses.contains(id)))
        .filter_map(|source_use| {
            Some((
                source_use.get("role")?.as_str()?,
                source_use.pointer("/location/indices")?,
            ))
        })
        .collect();
    assert!(
        addresses.contains(&("conversion", &json!(["0", "1"]))),
        "{addresses:?}"
    );
    assert!(
        addresses.contains(&("selector", &json!(["0", "0", "1"]))),
        "{addresses:?}"
    );
    let escaped = receipt
        .iter()
        .filter(|row| {
            row.pointer("/address/field").and_then(Value::as_str) == Some("attributeValue")
        })
        .any(|row| range(row, payload) == Some(b"&lt;&amp;"));
    assert!(
        escaped,
        "the attribute value range covers the actual XML escapes"
    );
    assert_eq!(
        receipt.last().ok_or("missing footer")?["type"],
        "end",
        "the receipt reports completion"
    );
    Ok(())
}

fn range<'bytes>(row: &Value, bytes: &'bytes [u8]) -> Option<&'bytes [u8]> {
    let address = &row["address"];
    let start = address["start"].as_str()?.parse::<usize>().ok()?;
    let end = address["endExclusive"].as_str()?.parse::<usize>().ok()?;
    bytes.get(start..end)
}

#[test]
fn text_stdout_stays_opaque_and_origin_is_opt_in() -> TestResult {
    let dir = tempfile::tempdir()?;
    let source = dir.path().join("source.xml");
    let durable = dir.path().join("stdout-origin.jsonl");
    let payload = b"\xef\xbb\xbf<a attr='value'>\r\n</a>";
    fs::write(&source, payload)?;
    let legacy = p4kx_testkit::p4kx()?.arg("cryxml").arg(&source).output()?;
    assert!(legacy.status.success(), "{legacy:?}");
    assert_eq!(legacy.stdout, payload);
    assert_eq!(legacy.stderr, b"");

    let command = p4kx_testkit::p4kx()?
        .arg("cryxml")
        .arg(&source)
        .arg("--origin-stderr")
        .arg("--origin-out")
        .arg(&durable)
        .output()?;
    assert!(command.status.success(), "{command:?}");
    assert_eq!(command.stdout, payload);
    assert_eq!(fs::read(durable)?, command.stderr);
    let receipt = rows(&command.stderr)?;
    let digest = measurement(payload)?;
    assert_eq!(
        receipt.first().ok_or("missing header")?["binding"],
        json!({
            "type": "stdout", "sha256": digest["sha256"], "byteLength": digest["byteLength"]
        })
    );
    let fields: Vec<_> = receipt
        .iter()
        .filter(|row| row["type"] == "field")
        .collect();
    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0]["address"], json!({"type": "opaquePayload"}));
    assert_eq!(fields[0]["containsEncryptedSource"], Value::Null);
    assert_eq!(receipt.last().ok_or("missing footer")?["fields"], "1");
    Ok(())
}

#[test]
fn in_place_conversion_keeps_its_observed_input_and_companion() -> TestResult {
    let dir = tempfile::tempdir()?;
    let source = dir.path().join("source.xml");
    let payload =
        TestElement::new("item").to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Big);
    fs::write(&source, &payload)?;
    let command = p4kx_testkit::p4kx()?
        .arg("cryxml")
        .arg(&source)
        .arg("--out")
        .arg(&source)
        .output()?;
    assert!(command.status.success(), "{command:?}");
    let converted = fs::read(&source)?;
    assert_eq!(
        converted,
        b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<item/>\n"
    );
    let receipt = rows(&fs::read(dir.path().join("source.xml.p4kx-origin.jsonl"))?)?;
    let source = receipt
        .iter()
        .find(|row| row["type"] == "source")
        .ok_or("source observation")?;
    assert_eq!(source["source"]["kind"], "file");
    assert_eq!(
        source["source"]["byteLength"]["value"],
        payload.len().to_string()
    );
    Ok(())
}

#[test]
fn an_input_alias_cannot_be_used_as_a_stdout_receipt() -> TestResult {
    let dir = tempfile::tempdir()?;
    let source = dir.path().join("source.xml");
    let alias = dir.path().join("receipt.jsonl");
    let payload = b"<untouched/>";
    fs::write(&source, payload)?;
    fs::hard_link(&source, &alias)?;
    let command = p4kx_testkit::p4kx()?
        .arg("cryxml")
        .arg(&source)
        .arg("--origin-out")
        .arg(&alias)
        .output()?;
    assert!(!command.status.success(), "{command:?}");
    assert_eq!(command.stdout, b"");
    assert_eq!(fs::read(&source)?, payload);
    assert_eq!(fs::read(&alias)?, payload);
    Ok(())
}
