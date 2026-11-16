// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_extraction::{Archive, ArchiveKey};
use p4kx_testkit::{ArchiveBuilder, CryXmlByteOrder, CryXmlSignature, TestElement, TestEntry};
use serde_json::{Value, json};

use super::{ClientProfileSettingsReport, GENERAL, VR, read_client_profile_settings};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const GENERAL_SOURCE: &str = "dAtA\\Libs\\Config\\Profiles\\default\\attributes.xml";
const GENERAL_XML: &str = concat!(
    "<Attributes>",
    "<Attr name='fixture_setting' value = ' 001&amp;&#x32; '/>",
    "<Attr name='fixture_setting' value=''/>",
    "<Note>a&amp;b</Note>",
    "</Attributes>",
);
#[path = "client_profile_failures_tests.rs"]
mod failures;

#[path = "client_profile_catalog_tests.rs"]
mod catalog;

#[path = "client_profile_namespaces_tests.rs"]
mod namespaces;

fn archive(
    entries: Vec<TestEntry>,
    key: Option<ArchiveKey>,
) -> TestResult<(tempfile::TempDir, Archive)> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Data.p4k");
    entries
        .into_iter()
        .fold(ArchiveBuilder::new(), ArchiveBuilder::entry)
        .write_to(&path)?;
    let archive = Archive::open_with_key(&path, key)?;
    Ok((directory, archive))
}

fn observe(
    entries: Vec<TestEntry>,
    key: Option<ArchiveKey>,
) -> TestResult<ClientProfileSettingsReport> {
    let (_directory, archive) = archive(entries, key)?;
    Ok(read_client_profile_settings(&archive, None)?)
}

fn at<'value>(value: &'value Value, pointer: &str) -> TestResult<&'value Value> {
    value
        .pointer(pointer)
        .ok_or_else(|| format!("missing profile report at {pointer}").into())
}

fn rows<'value>(value: &'value Value, pointer: &str) -> TestResult<&'value [Value]> {
    Ok(at(value, pointer)?
        .as_array()
        .ok_or("expected profile report array")?)
}

fn state(document: &Value, expected: &str, complete: bool) -> TestResult {
    assert_eq!(
        at(document, "/status/state")?,
        "present",
        "the input classification is explicit"
    );
    assert_eq!(
        at(document, "/status/value")?,
        expected,
        "the actual input state"
    );
    assert_eq!(
        at(document, "/status/sourceSetComplete")?,
        complete,
        "the decision's evidence completeness"
    );
    if expected != "read" {
        assert!(
            rows(document, "/elements")?.is_empty(),
            "no elements are fabricated for {expected}"
        );
    }
    Ok(())
}

fn inventory_row<'value>(
    report: &'value Value,
    table: &str,
    id: &Value,
) -> TestResult<&'value Value> {
    let mut matching = rows(report, table)?
        .iter()
        .filter(|row| row.get("id") == Some(id));
    let row = matching.next().ok_or("profile reference must resolve")?;
    assert!(
        matching.next().is_none(),
        "a profile use or source has one inventory row"
    );
    Ok(row)
}

fn immediate_use<'value>(report: &'value Value, field: &Value) -> TestResult<&'value Value> {
    let [id] = rows(field, "/sourceUsesRefs")? else {
        return Err("expected one actual origin".into());
    };
    inventory_row(report, "/provenance/uses", id)
}

fn coordinate(
    report: &Value,
    entry: &Value,
    field: &Value,
    encrypted: bool,
    role: &str,
    location: &Value,
) -> TestResult {
    assert_eq!(
        at(field, "/state")?,
        "present",
        "an explicitly empty value is present"
    );
    assert_eq!(
        at(field, "/containsEncryptedSource")?,
        encrypted,
        "the field's actual encryption"
    );
    assert_eq!(
        at(field, "/sourceSetComplete")?,
        true,
        "the field's evidence is complete"
    );
    let observed = immediate_use(report, field)?;
    assert_eq!(
        at(observed, "/scope")?,
        "payload",
        "the use comes from the decoded Entry"
    );
    assert_eq!(
        at(observed, "/source")?,
        at(entry, "/inputRef")?,
        "the selected physical Entry supplies the value"
    );
    assert_eq!(
        at(observed, "/role")?,
        role,
        "decoding and original spelling have independent roles"
    );
    assert_eq!(
        at(observed, "/location")?,
        location,
        "the exact original extent or slot survives"
    );
    let source = inventory_row(report, "/provenance/sources", at(observed, "/source")?)?;
    assert_eq!(
        at(source, "/entryIndex")?,
        at(entry, "/entryIndex")?,
        "the original directory position survives"
    );
    assert_eq!(
        at(source, "/path/value")?,
        at(entry, "/sourcePath/value")?,
        "the original path spelling survives"
    );
    Ok(())
}

fn location(namespace: &str, indices: &[&str], field: &str) -> Value {
    json!({"type":"subresource", "namespace":namespace, "indices":indices, "field":field})
}

fn text_location(namespace: &str, spelling: &str, field: &str) -> TestResult<Value> {
    let start = GENERAL_XML
        .find(spelling)
        .ok_or("unique fixture spelling")?;
    Ok(location(
        namespace,
        &[&start.to_string(), &(start + spelling.len()).to_string()],
        field,
    ))
}

fn binary_profile() -> Vec<u8> {
    fn setting(value: &str) -> TestElement {
        TestElement::new("Attr")
            .attribute("name", "fixture_setting")
            .attribute("value", value)
    }

    TestElement::new("Attributes")
        .child(setting("001.50"))
        .child(setting("").content("raw&unchanged"))
        .to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little)
}

#[test]
fn profile_fields_retain_original_text_extents_and_encrypted_cryxml_slots() -> TestResult {
    let report = observe(
        vec![
            TestEntry::stored(GENERAL_SOURCE, GENERAL_XML),
            TestEntry::stored(VR, binary_profile()).encrypted(),
        ],
        Some(p4kx_testkit::test_key()),
    )?;
    assert_eq!(
        report.settings().decoded_documents(),
        2,
        "both source dialects are readable"
    );
    assert!(
        !report.settings().has_failed_inputs(),
        "both profiles were decoded"
    );
    let document = serde_json::to_value(report)?;
    let schema = serde_json::to_value(schemars::schema_for!(ClientProfileSettingsReport))?;
    jsonschema::validator_for(&schema)?
        .validate(&document)
        .map_err(|error| format!("client profile report schema: {error}"))?;
    let [general, vr] = rows(&document, "/settings/documents")? else {
        return Err("general followed by VR".into());
    };
    state(general, "read", true)?;
    state(vr, "read", true)?;
    check_text(&document, general)?;
    check_binary(&document, vr)
}

fn check_text(report: &Value, document: &Value) -> TestResult {
    assert_eq!(
        at(document, "/kind")?,
        "general",
        "the General kind is explicit"
    );
    assert_eq!(
        at(document, "/requestedPath")?,
        GENERAL,
        "canonical General path"
    );
    let [entry] = rows(document, "/candidates")? else {
        return Err("one physical candidate for General".into());
    };
    assert_eq!(
        at(entry, "/sourcePath/value")?,
        GENERAL_SOURCE,
        "filename spelling survives"
    );
    let [root, first, second, note] = rows(document, "/elements")? else {
        return Err("root, repeated settings and Note in their authored order".into());
    };
    assert_eq!(
        at(root, "/name/value")?,
        "Attributes",
        "the original root name survives"
    );
    assert!(
        at(root, "/parent/value")?.is_null(),
        "the original root has no parent"
    );
    for element in [first, second, note] {
        assert_eq!(
            at(element, "/parent/value")?,
            0,
            "each witness is a child of the original root"
        );
    }
    for element in [first, second] {
        assert_eq!(
            at(element, "/declarations/0/value/value/value")?,
            "fixture_setting",
            "duplicate setting names survive"
        );
    }
    check_text_values(report, entry, first, second, note)
}

fn check_text_values(
    report: &Value,
    entry: &Value,
    first: &Value,
    second: &Value,
    note: &Value,
) -> TestResult {
    let first_value = at(first, "/declarations/1/value")?;
    let span = text_location("textXmlAttribute", "value = ' 001&amp;&#x32; '", "value")?;
    check_lexeme(
        report,
        entry,
        first_value,
        " 001&2 ",
        " 001&amp;&#x32; ",
        &span,
    )?;
    let empty = at(second, "/declarations/1/value")?;
    assert_eq!(
        at(empty, "/value/value")?,
        "",
        "explicit empty setting remains present"
    );
    assert!(
        at(empty, "/lexeme")?.is_null(),
        "there is no alternate spelling"
    );
    let span = text_location("textXmlAttribute", "value=''", "value")?;
    coordinate(report, entry, at(empty, "/value")?, false, "payload", &span)?;

    let content = at(note, "/content/0")?;
    let span = text_location("textXmlContent", "a&amp;b", "$content")?;
    check_lexeme(report, entry, content, "a&b", "a&amp;b", &span)
}

fn check_lexeme(
    report: &Value,
    entry: &Value,
    property: &Value,
    decoded: &str,
    spelling: &str,
    span: &Value,
) -> TestResult {
    assert_eq!(
        at(property, "/value/value")?,
        decoded,
        "the decoded text retains formatting"
    );
    assert_eq!(
        at(property, "/lexeme/value")?,
        spelling,
        "the original value spelling is exact"
    );
    coordinate(
        report,
        entry,
        at(property, "/value")?,
        false,
        "conversion",
        span,
    )?;
    coordinate(
        report,
        entry,
        at(property, "/lexeme")?,
        false,
        "payload",
        span,
    )
}

fn check_binary(report: &Value, document: &Value) -> TestResult {
    assert_eq!(
        at(document, "/kind")?,
        "virtualReality",
        "the VR kind is explicit"
    );
    assert_eq!(at(document, "/requestedPath")?, VR, "canonical VR path");
    let [entry] = rows(document, "/candidates")? else {
        return Err("one physical candidate for VR".into());
    };
    let [_, first, second] = rows(document, "/elements")? else {
        return Err("both authored CryXml settings".into());
    };
    for (element, node, slot, expected) in [(first, "1", "1", "001.50"), (second, "2", "3", "")] {
        assert_eq!(
            at(element, "/declarations/0/value/value/value")?,
            "fixture_setting",
            "duplicate CryXml names survive"
        );
        let property = at(element, "/declarations/1/value")?;
        assert_eq!(
            at(property, "/value/value")?,
            expected,
            "exact CryXml value"
        );
        assert!(
            at(property, "/lexeme")?.is_null(),
            "there is no alternate spelling"
        );
        let span = location("cryxmlAttribute", &[node, slot], "value");
        coordinate(
            report,
            entry,
            at(property, "/value")?,
            true,
            "payload",
            &span,
        )?;
    }
    let content = at(second, "/content/0/value")?;
    assert_eq!(
        at(content, "/value")?,
        "raw&unchanged",
        "original CryXml content"
    );
    let span = location("cryxmlContent", &["2"], "$content");
    coordinate(report, entry, content, true, "payload", &span)
}
