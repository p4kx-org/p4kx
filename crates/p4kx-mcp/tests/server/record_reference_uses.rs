// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use crate::forge::AEGS_TEXT;
use crate::model_item_origin_checks::{field, marker, uses};
use crate::support::TestResult;

pub(super) fn direct(page: &Value, original: &[u8]) -> TestResult {
    const GUID: &str = "records/0/maker/guid";
    const PATH: &str = "records/0/maker/path";
    const TYPE: &str = "records/0/maker/type";
    const STORED_GUID: [u8; 16] = [
        0x6d, 0x4c, 0x2b, 0x1f, 0x3c, 0x0e, 0x8d, 0x5a, 0x5f, 0x4e, 0x3d, 0x2c, 0x1b, 0x0a, 0x9f,
        0x8e,
    ];
    assert_eq!(
        page.pointer("/records/0/maker/guid"),
        Some(&json!(AEGS_TEXT)),
        "the stored Reference GUID is rendered"
    );
    let guid = only_payload(page, GUID)?;
    assert!(is_bytes(guid, "slot", "payload"), "the GUID's own slot");
    assert_eq!(
        super::source_span(guid, original)?,
        STORED_GUID,
        "only the exact stored GUID suffix contributes data"
    );
    for output in [PATH, TYPE] {
        marker(field(page, output)?, true);
        let selector = source_bytes(page, output, "slot", "selector")?;
        assert_eq!(
            selector.pointer("/location/indices"),
            guid.pointer("/location/indices"),
            "{output} uses the same GUID suffix to resolve its target"
        );
    }
    assert_record_role(page, PATH, "fileName", "conversion")?;
    assert_eq!(
        super::source_span(source_bytes(page, PATH, "text", "payload")?, original)?,
        b"libs/foundry/records/manufacturers/aegs.xml",
        "the selected record filename supplies the path text"
    );
    assert_record_role(page, TYPE, "sourceType", "selector")?;
    assert_struct_name(only_payload(page, TYPE)?, "0");
    Ok(())
}

pub(super) fn weak(page: &Value, original: &[u8]) -> TestResult {
    const GUID: &str = "records/0/selected/guid";
    const PATH: &str = "records/0/selected/path";
    const TYPE: &str = "records/0/selected/type";
    const AT: &str = "records/0/selected/at";
    for output in [GUID, PATH, TYPE, AT] {
        marker(field(page, output)?, false);
        assert_eq!(
            super::source_span(source_bytes(page, output, "slot", "selector")?, original)?,
            [0, 0, 0, 0, 1, 0, 0, 0],
            "{output} selects the second Item with the original WeakPointer"
        );
        assert_eq!(
            super::source_span(
                source_bytes(page, output, "arrayHeader", "selector")?,
                original
            )?,
            [2, 0, 0, 0, 0, 0, 0, 0],
            "{output} follows the original Class array header to the selected element"
        );
    }
    let guid = only_payload(page, GUID)?;
    assert!(
        is_record_role(guid, "guid", "payload"),
        "the actual owner Record supplies the resolved GUID"
    );
    assert_eq!(
        page.pointer("/records/0/selected/guid"),
        page.pointer("/records/0/$guid"),
        "the weak link resolves to the owner Record's GUID"
    );
    assert_struct_name(only_payload(page, TYPE)?, "0");
    assert_record_role(page, PATH, "fileName", "conversion")?;
    assert_eq!(
        super::source_span(only_payload(page, PATH)?, original)?,
        b"holder.xml",
        "the actual owner Record filename supplies the path"
    );
    assert_eq!(
        super::source_span(source_bytes(page, AT, "text", "conversion")?, original)?,
        b"children",
        "the original field name contributes to the owner instance path"
    );
    Ok(())
}

pub(super) fn dangling(page: &Value, original: &[u8]) -> TestResult {
    const GUID: &str = "records/0/invalid/guid";
    const ERROR: &str = "records/0/invalid/error";
    const MISSING_GUID: &str = "00000000-0000-0000-0000-000000000077";
    const STORED_GUID: [u8; 16] = [0, 0, 0, 0, 0, 0, 0, 0, 0x77, 0, 0, 0, 0, 0, 0, 0];
    assert_eq!(
        page.pointer("/records/0/invalid"),
        Some(&json!({
            "guid": MISSING_GUID,
            "error": format!("the Reference refers to the Record {MISSING_GUID}, which does not exist"),
        })),
        "the canonical missing-target error and its original GUID"
    );
    marker(field(page, GUID)?, false);
    let guid = only_payload(page, GUID)?;
    assert!(
        is_bytes(guid, "slot", "payload"),
        "the original Reference slot"
    );
    assert_eq!(
        super::source_span(guid, original)?,
        STORED_GUID,
        "the dangling GUID retains only the 16 authored GUID bytes"
    );
    let error = field(page, ERROR)?;
    assert_eq!(
        error.get("sourceSetComplete"),
        Some(&json!(false)),
        "the absence of a lookup target has no complete proof"
    );
    assert_eq!(
        error.get("containsEncryptedSource"),
        Some(&Value::Null),
        "an incomplete unencrypted source set cannot settle the encryption marker"
    );
    let slot = source_bytes(page, ERROR, "slot", "conversion")?;
    assert_eq!(
        slot.get("location"),
        guid.get("location"),
        "the error uses the same original bytes to render the GUID in its message"
    );
    Ok(())
}

fn assert_struct_name(site: &Value, index: &str) {
    assert_eq!(
        site.pointer("/location/namespace").and_then(Value::as_str),
        Some("dataforgeStructDefinition"),
        "the selected struct name is the type payload"
    );
    assert_eq!(
        site.pointer("/location/field"),
        Some(&json!("name")),
        "name"
    );
    assert_eq!(
        site.pointer("/location/indices"),
        Some(&json!([index])),
        "the original target struct definition"
    );
}

fn only_payload<'a>(page: &'a Value, output: &str) -> TestResult<&'a Value> {
    let sites = uses(page, output)?;
    let mut payloads = sites
        .into_iter()
        .filter(|site| site.get("role").and_then(Value::as_str) == Some("payload"));
    let payload = payloads
        .next()
        .ok_or_else(|| format!("{output} has no payload"))?;
    assert!(payloads.next().is_none(), "{output} has only one payload");
    Ok(payload)
}

fn assert_record_role(page: &Value, output: &str, label: &str, role: &str) -> TestResult {
    assert!(
        uses(page, output)?
            .iter()
            .any(|site| is_record_role(site, label, role)),
        "{output} has original Record 0 {label} as {role}"
    );
    Ok(())
}

fn is_record_role(site: &Value, label: &str, role: &str) -> bool {
    site.get("role").and_then(Value::as_str) == Some(role)
        && site.pointer("/location/type").and_then(Value::as_str) == Some("dataForge")
        && site
            .pointer("/location/recordIndex")
            .and_then(Value::as_u64)
            == Some(0)
        && site.pointer("/location/field").and_then(Value::as_str) == Some(label)
}

fn source_bytes<'a>(
    page: &'a Value,
    output: &str,
    label: &str,
    role: &str,
) -> TestResult<&'a Value> {
    uses(page, output)?
        .into_iter()
        .find(|site| is_bytes(site, label, role))
        .ok_or_else(|| format!("{output} has no original {label} span as {role}").into())
}

fn is_bytes(site: &Value, label: &str, role: &str) -> bool {
    site.get("role").and_then(Value::as_str) == Some(role)
        && site.pointer("/location/namespace").and_then(Value::as_str)
            == Some("dataforgeDecodedBytes")
        && site.pointer("/location/field").and_then(Value::as_str) == Some(label)
}
