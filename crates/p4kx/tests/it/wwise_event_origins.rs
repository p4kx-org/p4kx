// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_testkit::{ArchiveBuilder, TestAtl, TestEntry};
use serde_json::{Value, json};

use super::TestResult;

const ATL: &str = "Data\\Libs\\GameAudio\\SC_Duplicate.xml";
const EVENT: &str = "Emit_Shared";
const MALFORMED_CLEAR_WORD: [u8; 4] = [0, 0, 9, 9];

#[test]
fn json_retains_physical_atl_paths_and_original_xml_sites() -> TestResult {
    let fixtures = [
        Definition::new("Play_Clear", Some(false), false),
        Definition::new("Play_Encrypted", Some(true), true),
        Definition::new("Play_Unknown", None, false),
    ];
    let directory = tempfile::tempdir()?;
    let archive = directory.path().join("Data.p4k");
    let builder = fixtures
        .iter()
        .fold(ArchiveBuilder::new(), |builder, fixture| {
            let entry = TestEntry::stored(ATL, fixture.xml.as_bytes());
            builder.entry(match fixture.encryption {
                Some(false) => entry,
                Some(true) => entry.encrypted(),
                None => entry.with_encryption_block(MALFORMED_CLEAR_WORD),
            })
        });
    builder.write_to(&archive)?;

    let output = p4kx_testkit::p4kx()?
        .args(["--json", "wwise", "events"])
        .arg(&archive)
        .args(["--search", EVENT, "--no-entities"])
        .output()?;
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let document: Value = serde_json::from_slice(&output.stdout)?;
    let rows = document
        .get("triggers")
        .and_then(Value::as_array)
        .ok_or("matched ATL definitions")?;
    assert_eq!(
        rows.len(),
        fixtures.len(),
        "all three physical definitions were read"
    );
    for (physical, fixture) in fixtures.iter().enumerate() {
        let ordinal = rows
            .iter()
            .position(|row| text_at(row, "/name") == Some(fixture.name))
            .ok_or("selected ATL definition")?;
        check_definition(&document, ordinal, physical, fixture)?;
    }
    Ok(())
}

struct Definition {
    name: &'static str,
    encryption: Option<bool>,
    localised: bool,
    xml: String,
}

impl Definition {
    fn new(name: &'static str, encryption: Option<bool>, localised: bool) -> Self {
        let config = TestAtl::new("SC_Duplicate");
        let atl = if localised {
            config.localised_trigger(name, EVENT)
        } else {
            config.trigger(name, &[EVENT])
        };
        Self {
            name,
            encryption,
            localised,
            xml: atl.to_xml(),
        }
    }
}

fn check_definition(
    document: &Value,
    ordinal: usize,
    physical: usize,
    fixture: &Definition,
) -> TestResult {
    let row_path = format!("/triggers/{ordinal}");
    let row = document.pointer(&row_path).ok_or("ATL output row")?;
    assert_eq!(
        text_at(row, "/definition"),
        Some(ATL),
        "the shared Entry path"
    );
    assert_eq!(
        text_at(row, "/events/0/name"),
        Some(EVENT),
        "the shared authored event name"
    );
    assert_eq!(
        row.pointer("/events/0/localised").and_then(Value::as_bool),
        Some(fixture.localised),
        "the projected event localisation"
    );
    assert_eq!(
        row.get("matched_trigger").and_then(Value::as_bool),
        Some(false),
        "the event supplied the match"
    );

    let site = contributor(
        document,
        &format!("{row_path}/definition"),
        Some(false),
        physical,
    )?;
    assert_eq!(
        text_at(site, "/scope"),
        Some("archiveDirectoryMetadata"),
        "directory scope"
    );
    assert_eq!(
        text_at(site, "/role"),
        Some("payload"),
        "the original Entry name"
    );
    assert_eq!(
        site.get("location"),
        Some(&json!({"type": "metadata", "field": "path"})),
        "path binding"
    );

    for (suffix, attribute, value) in [
        ("name", "atl_name", fixture.name),
        ("events/0/name", "wwise_name", EVENT),
    ] {
        let path = format!("{row_path}/{suffix}");
        let site = contributor(document, &path, fixture.encryption, physical)?;
        assert_site(site, "payload", "textXmlAttribute", attribute);
        assert_eq!(
            source_text(&fixture.xml, site)?,
            format!(r#"{attribute}="{value}""#),
            "original attribute at {path}"
        );
    }
    check_localised(document, &row_path, physical, fixture)
}

fn check_localised(
    document: &Value,
    row: &str,
    physical: usize,
    fixture: &Definition,
) -> TestResult {
    let site = contributor(
        document,
        &format!("{row}/events/0/localised"),
        fixture.encryption,
        physical,
    )?;
    let source = source_text(&fixture.xml, site)?;
    if fixture.localised {
        assert_site(site, "conversion", "textXmlAttribute", "wwise_localised");
        assert_eq!(
            source, r#"wwise_localised="true""#,
            "the original true attribute"
        );
    } else {
        assert_site(site, "conversion", "textXmlElement", "WwiseEvent");
        assert!(
            source.starts_with("<WwiseEvent") && !source.contains("wwise_localised"),
            "the original event element has no localisation attribute"
        );
    }
    Ok(())
}

fn assert_site(site: &Value, role: &str, namespace: &str, field: &str) {
    assert_eq!(
        text_at(site, "/scope"),
        Some("payload"),
        "payload scope for {field}"
    );
    assert_eq!(
        text_at(site, "/role"),
        Some(role),
        "source-use role for {field}"
    );
    assert_eq!(
        text_at(site, "/location/namespace"),
        Some(namespace),
        "XML location for {field}"
    );
    assert_eq!(
        text_at(site, "/location/field"),
        Some(field),
        "exact original XML field"
    );
}

fn contributor<'report>(
    document: &'report Value,
    path: &str,
    marker: Option<bool>,
    physical: usize,
) -> Result<&'report Value, Box<dyn Error>> {
    let origin = document
        .get("fieldOrigins")
        .and_then(|fields| fields.get(path))
        .ok_or("immediate field origin")?;
    assert_eq!(
        origin.get("containsEncryptedSource"),
        Some(&json!(marker)),
        "{path}"
    );
    assert_eq!(
        origin.get("sourceSetComplete").and_then(Value::as_bool),
        Some(true),
        "{path}"
    );
    let refs = origin
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("source references")?;
    let [reference] = refs.as_slice() else {
        return Err(format!("expected one direct contributor at {path}").into());
    };
    let site = document
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .and_then(|uses| uses.iter().find(|site| site.get("id") == Some(reference)))
        .ok_or("original use")?;
    let source_id = site.get("source").ok_or("physical source id")?;
    let entry = document
        .pointer("/provenance/sources")
        .and_then(Value::as_array)
        .and_then(|sources| {
            sources
                .iter()
                .find(|entry| entry.get("id") == Some(source_id))
        })
        .ok_or("original physical source")?;
    assert_eq!(
        entry.get("entryIndex"),
        Some(&json!(physical)),
        "physical Entry for {path}"
    );
    assert_eq!(
        text_at(entry, "/path/value"),
        Some(ATL),
        "the colliding path for {path}"
    );
    Ok(site)
}

fn text_at<'value>(value: &'value Value, pointer: &str) -> Option<&'value str> {
    value.pointer(pointer).and_then(Value::as_str)
}

fn source_text<'xml>(xml: &'xml str, site: &Value) -> Result<&'xml str, Box<dyn Error>> {
    let indices = site
        .pointer("/location/indices")
        .and_then(Value::as_array)
        .ok_or("XML source byte range")?;
    let [start, end] = indices.as_slice() else {
        return Err("text XML coordinates must have two byte offsets".into());
    };
    let start: usize = start.as_str().ok_or("XML start")?.parse()?;
    let end: usize = end.as_str().ok_or("XML end")?.parse()?;
    xml.get(start..end)
        .ok_or_else(|| "original XML source range".into())
}
