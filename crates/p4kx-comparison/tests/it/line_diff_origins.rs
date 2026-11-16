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
fn converted_cryxml_keeps_exact_bytes_when_leaf_shape_is_unavailable() -> TestResult {
    let bytes =
        TestElement::new("Original").to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little);
    let name = original_string(&bytes, "Original")?;
    let (_dir, old, new) = pair(Vec::new(), vec![TestEntry::stored(PATH, bytes).encrypted()])?;
    let report = serde_json::to_value(line_diff(&old, &new, PATH, DEFAULT_CONTEXT)?)?;
    assert_origin(&report, "/old", false)?;
    assert_origin(&report, "/content", true)?;
    let declaration = assert_origin(&report, "/hunks/0/lines/0/text", false)?;
    assert!(
        declaration
            .iter()
            .all(|usage| usage.get("scope") != Some(&json!("payload"))),
        "the fixed XML declaration has no authored payload bytes"
    );
    let element = "/hunks/0/lines/1/text";
    let line = field(&report, element)?;
    assert_eq!(
        line.get("containsEncryptedSource"),
        Some(&json!(true)),
        "the name originates in encrypted input"
    );
    assert_eq!(
        line.get("sourceSetComplete"),
        Some(&json!(false)),
        "the unknown leaf shape remains incomplete"
    );
    assert!(
        uses(&report, line)?.iter().any(|usage| {
            usage.get("role") == Some(&json!("conversion"))
                && usage.pointer("/location/type") == Some(&json!("opaque"))
        }),
        "an unavailable structural contribution stays explicitly opaque"
    );
    raw_use(&report, element, "conversion", "stringBytes", name)?;
    Ok(())
}

#[test]
fn cryxml_hunks_bind_original_operands_only_for_the_emitted_side_and_line() -> TestResult {
    const OLD_VALUE: &str = "old<&";
    const NEW_VALUE: &str = "new<&";
    const OMITTED: &str = "unchanged-payload";
    let element = |value| {
        TestElement::new("Options")
            .attribute("label", value)
            .child(TestElement::new("Child").attribute("ignored", OMITTED))
    };
    let old_bytes = element(OLD_VALUE).to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little);
    let new_bytes = element(NEW_VALUE).to_cryxml(CryXmlSignature::Cry3Sdk, CryXmlByteOrder::Big);
    let old_range = original_string(&old_bytes, OLD_VALUE)?;
    let new_range = original_string(&new_bytes, NEW_VALUE)?;
    let omitted_old = original_string(&old_bytes, OMITTED)?;
    let omitted_new = original_string(&new_bytes, OMITTED)?;
    let (_dir, old, new) = pair(
        vec![TestEntry::stored(PATH, old_bytes)],
        vec![TestEntry::stored(PATH, new_bytes).encrypted()],
    )?;
    let report = serde_json::to_value(line_diff(&old, &new, PATH, 0)?)?;
    let removed = "/hunks/0/lines/0/text";
    let added = "/hunks/0/lines/1/text";
    assert_eq!(
        report.pointer(removed),
        Some(&json!("<Options label=\"old&lt;&amp;\">\n")),
        "the removed line keeps the original XML rendering"
    );
    assert_eq!(
        report.pointer(added),
        Some(&json!("<Options label=\"new&lt;&amp;\">\n")),
        "the added line keeps the original XML rendering"
    );
    assert_origin(&report, removed, false)?;
    assert_origin(&report, added, true)?;
    let old_value = raw_use(&report, removed, "conversion", "stringBytes", old_range)?;
    let new_value = raw_use(&report, added, "conversion", "stringBytes", new_range)?;
    assert_ne!(
        old_value.get("source"),
        new_value.get("source"),
        "removed and added operands belong to different physical inputs"
    );
    assert!(
        report
            .pointer("/provenance/uses")
            .and_then(Value::as_array)
            .ok_or("missing source inventory")?
            .iter()
            .all(|usage| {
                usage.pointer("/location/field") != Some(&json!("stringBytes"))
                    || ![json_range(&omitted_old), json_range(&omitted_new)]
                        .iter()
                        .any(|range| usage.pointer("/location/indices") == Some(range))
            }),
        "unchanged CryXml lines beyond context add no original byte evidence"
    );
    Ok(())
}

fn original_string(bytes: &[u8], value: &str) -> Result<std::ops::Range<usize>, Box<dyn Error>> {
    let mut stored = value.as_bytes().to_vec();
    stored.push(0);
    let start = bytes
        .windows(stored.len())
        .position(|window| window == stored)
        .ok_or("missing original NUL-terminated string")?;
    Ok(start..start + value.len())
}

fn json_range(range: &std::ops::Range<usize>) -> Value {
    json!([range.start.to_string(), range.end.to_string()])
}

fn raw_use<'a>(
    report: &'a Value,
    path: &str,
    role: &str,
    kind: &str,
    range: std::ops::Range<usize>,
) -> Result<&'a Value, Box<dyn Error>> {
    let interval = json_range(&range);
    uses(report, field(report, path)?)?
        .into_iter()
        .find(|usage| {
            usage.get("role") == Some(&json!(role))
                && usage.pointer("/location/namespace") == Some(&json!("entryDecodedBytes"))
                && usage.pointer("/location/field") == Some(&json!(kind))
                && usage.pointer("/location/indices") == Some(&interval)
        })
        .ok_or_else(|| {
            format!("missing original {role} {kind} interval {range:?} for {path}").into()
        })
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
