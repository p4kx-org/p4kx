// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_comparison::{DEFAULT_CONTEXT, MAX_DIFF_LEN, line_diff};
use p4kx_testkit::{CryXmlByteOrder, CryXmlSignature, TestElement, TestEntry};
use serde_json::{Value, json};

use crate::common::{TestResult, pair};

const PATH: &str = "Data\\one.txt";

#[test]
fn text_diff_binds_emitted_lines_and_alignment_to_distinct_inputs() -> TestResult {
    let utf16: Vec<_> = [0xff, 0xfe]
        .into_iter()
        .chain(
            "same\r\nnew\u{1f600}\r\n"
                .encode_utf16()
                .flat_map(u16::to_le_bytes),
        )
        .collect();
    let (_dir, old, new) = pair(
        vec![TestEntry::stored(PATH, b"same\r\nold\r\n")],
        vec![TestEntry::stored(PATH, utf16).encrypted()],
    )?;
    let report = serde_json::to_value(line_diff(&old, &new, PATH, DEFAULT_CONTEXT)?)?;
    let removed = "/hunks/0/lines/1";
    let added = "/hunks/0/lines/2";
    assert_eq!(
        report.pointer(&format!("{removed}/text")),
        Some(&json!("old\r\n"))
    );
    assert_eq!(
        report.pointer(&format!("{added}/text")),
        Some(&json!("new\u{1f600}\r\n"))
    );
    assert_origin(&report, "/path", false)?;
    assert_origin(&report, "/new/size", false)?;
    assert_origin(&report, "/content", true)?;
    assert_origin(&report, &format!("{removed}/kind"), true)?;
    assert_origin(&report, &format!("{removed}/text"), false)?;
    let added_text = assert_origin(&report, &format!("{added}/text"), true)?;
    assert!(
        added_text.iter().any(|usage| {
            usage.get("role") == Some(&json!("selector"))
                && usage.pointer("/location/field") == Some(&json!("byteOrderMark"))
                && usage.pointer("/location/indices") == Some(&json!(["0", "2"]))
        }),
        "UTF-16 byte order is selected by the actual BOM"
    );
    assert_range(&report, &format!("{removed}/text"), "payload", ["6", "11"])?;
    assert_range(
        &report,
        &format!("{added}/text"),
        "conversion",
        ["14", "28"],
    )?;
    Ok(())
}

#[test]
fn an_encrypted_unread_entry_uses_the_declared_size_of_its_directory() -> TestResult {
    let (_dir, old, new) = pair(
        vec![
            TestEntry::stored(PATH, b"a")
                .encrypted()
                .with_uncompressed_size(MAX_DIFF_LEN + 1),
        ],
        vec![TestEntry::stored(PATH, b"b").encrypted()],
    )?;
    let report = serde_json::to_value(line_diff(&old, &new, PATH, DEFAULT_CONTEXT)?)?;
    assert_eq!(report.get("content"), Some(&json!("too_large")));
    let decision = assert_origin(&report, "/content", false)?;
    assert!(
        decision
            .iter()
            .any(|usage| { usage.pointer("/location/field") == Some(&json!("uncompressed_size")) })
    );
    assert!(
        decision
            .iter()
            .all(|usage| usage.get("scope") == Some(&json!("archiveDirectoryMetadata")))
    );
    assert_origin(&report, "/new/encoding", true)?;
    Ok(())
}

#[test]
fn converted_cryxml_lines_keep_the_encrypted_payload_with_unknown_subspan() -> TestResult {
    let bytes =
        TestElement::new("Original").to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little);
    let (_dir, old, new) = pair(Vec::new(), vec![TestEntry::stored(PATH, bytes).encrypted()])?;
    let report = serde_json::to_value(line_diff(&old, &new, PATH, DEFAULT_CONTEXT)?)?;
    assert_origin(&report, "/old", false)?;
    assert_origin(&report, "/content", true)?;
    let line = field(&report, "/hunks/0/lines/0/text")?;
    assert_eq!(line.get("containsEncryptedSource"), Some(&json!(true)));
    assert_eq!(line.get("sourceSetComplete"), Some(&json!(false)));
    assert!(uses(&report, line)?.iter().any(|usage| {
        usage.get("role") == Some(&json!("conversion"))
            && usage.pointer("/location/type") == Some(&json!("opaque"))
    }));
    Ok(())
}

fn assert_range(report: &Value, path: &str, role: &str, expected: [&str; 2]) -> TestResult {
    let field = field(report, path)?;
    assert!(
        uses(report, field)?.iter().any(|usage| {
            usage.get("role") == Some(&json!(role))
                && usage.pointer("/location/namespace") == Some(&json!("entryDecodedBytes"))
                && usage.pointer("/location/indices") == Some(&json!(expected))
        }),
        "original {role} byte interval {expected:?} for {path}"
    );
    Ok(())
}

fn assert_origin<'a>(
    report: &'a Value,
    path: &str,
    encrypted: bool,
) -> Result<Vec<&'a Value>, Box<dyn Error>> {
    let field = field(report, path)?;
    assert_eq!(
        field.get("containsEncryptedSource"),
        Some(&json!(encrypted)),
        "{path}"
    );
    assert_eq!(field.get("sourceSetComplete"), Some(&json!(true)), "{path}");
    let observations = uses(report, field)?;
    assert!(!observations.is_empty(), "evidenced origin for {path}");
    Ok(observations)
}

fn field<'a>(report: &'a Value, path: &str) -> Result<&'a Value, Box<dyn Error>> {
    report
        .get("fieldOrigins")
        .and_then(|fields| fields.get(path))
        .ok_or_else(|| format!("missing {path}").into())
}

fn uses<'a>(report: &'a Value, field: &Value) -> Result<Vec<&'a Value>, Box<dyn Error>> {
    let all = report
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("missing source inventory")?;
    field
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("missing source use list")?
        .iter()
        .map(|id| {
            all.iter()
                .find(|usage| usage.get("id") == Some(id))
                .ok_or_else(|| format!("unresolved {id}").into())
        })
        .collect()
}
