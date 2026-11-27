// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;
use std::error::Error;
use std::ops::Range;

use serde_json::{Value, json};

use super::TestResult;

pub(super) fn check_unknown(origin: &Value) {
    assert_eq!(
        origin,
        &json!({
            "containsEncryptedSource": null,
            "sourceUsesRefs": [],
            "sourceSetComplete": false
        }),
        "no bound source proof remains explicitly unknown"
    );
}

pub(super) fn check_plain_fields(
    report: &Value,
    code: &str,
    namespace: &str,
    expected: &[&str],
) -> TestResult {
    let file = report.pointer("/txtp/files/0").ok_or("TXTP file report")?;
    let index = file
        .get("approximations")
        .and_then(Value::as_array)
        .ok_or("approximation codes")?
        .iter()
        .position(|value| value.as_str() == Some(code))
        .ok_or_else(|| format!("the reported code {code}"))?;
    let origin = file
        .get("fieldOrigins")
        .and_then(|fields| fields.get(format!("/approximations/{index}")))
        .ok_or_else(|| format!("origin of {code}"))?;
    assert_eq!(
        origin
            .get("containsEncryptedSource")
            .and_then(Value::as_bool),
        Some(false),
        "{code}: actual plain original source"
    );
    assert_eq!(
        origin.get("sourceSetComplete").and_then(Value::as_bool),
        Some(true),
        "{code}: all contributors are known"
    );
    let uses = file
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("source uses")?;
    let refs = origin
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("source use references")?;
    check_fields(uses, refs, namespace, expected)
}

fn check_fields(uses: &[Value], refs: &[Value], namespace: &str, expected: &[&str]) -> TestResult {
    let mut fields = BTreeSet::new();
    let mut layout_selectors = 0;
    for reference in refs {
        let original = uses
            .iter()
            .find(|observation| observation.get("id") == Some(reference))
            .ok_or("an exact original use")?;
        let use_namespace = original
            .pointer("/location/namespace")
            .and_then(Value::as_str);
        let field = original.pointer("/location/field").and_then(Value::as_str);
        match original.get("role").and_then(Value::as_str) {
            Some("conversion") => {
                assert_eq!(
                    use_namespace,
                    Some(namespace),
                    "the original declaration's domain"
                );
                let field = field.ok_or("source field name")?;
                assert!(
                    fields.insert(field),
                    "the original field {field} is cited once"
                );
            }
            Some("selector") => {
                assert_eq!(use_namespace, Some("bnkBkhd"), "the bank layout namespace");
                assert_eq!(field, Some("version"), "the selected version field");
                layout_selectors += 1;
            }
            _ => return Err("unexpected origin source role".into()),
        }
    }
    assert_eq!(
        fields,
        expected.iter().copied().collect::<BTreeSet<_>>(),
        "the exact diagnostic input field names"
    );
    assert_eq!(layout_selectors, 1, "one exact layout selector");
    Ok(())
}

pub(super) fn check_address(
    row: &Value,
    bytes: &[u8],
    value: &str,
    origin: &Value,
) -> Result<Range<usize>, Box<dyn Error>> {
    let address = &row["address"];
    assert_eq!(
        address["type"], "textBytes",
        "address uses the actual text span"
    );
    let start = address["start"]
        .as_str()
        .ok_or("start byte")?
        .parse::<usize>()?;
    let end = address["endExclusive"]
        .as_str()
        .ok_or("end byte")?
        .parse::<usize>()?;
    assert_eq!(
        bytes.get(start..end),
        Some(value.as_bytes()),
        "exact field bytes"
    );
    for key in [
        "containsEncryptedSource",
        "sourceSetComplete",
        "sourceUsesRefs",
    ] {
        assert_eq!(&row[key], &origin[key], "report and sidecar share {key}");
    }
    Ok(start..end)
}
