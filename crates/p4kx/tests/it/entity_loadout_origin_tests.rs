// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntity, TestEntry, TestLoadout, TestLoadoutEntry,
    entity_schema,
};
use serde_json::Value;

use super::TestResult;

#[path = "entity_loadout_attachment_origin_tests.rs"]
mod attachments;
#[path = "entity_loadout_definition_origin_tests.rs"]
mod definitions;
#[path = "entity_loadout_vehicle_selection_tests.rs"]
mod vehicle_selection;
#[path = "entity_loadout_vehicle_visibility_tests.rs"]
mod vehicle_visibility;

const BASE_PATH: &str = "Data\\Loadouts\\Base.xml";
const LOCAL_PATH: &str = "Data\\Loadouts\\Root.xml";
const BASE: &str = concat!(
    "\u{feff}",
    "<Loadout><Items><Item portName='left' itemName='Red'/>",
    "<Item portName='right' itemName='Red'/></Items></Loadout>",
);
const LOCAL: &str = concat!(
    "<Loadout><Loadouts><Loadout loadout='Loadouts/Base.xml'/></Loadouts>",
    "<Items><Item portName='right' itemName='Blue'/></Items></Loadout>",
);

#[test]
fn json_names_keep_exact_mixed_sources_and_the_winning_loadout_occurrence() -> TestResult {
    let directory = tempfile::tempdir()?;
    let archive = directory.path().join("Data.p4k");
    let record = TestEntity::new(1, "Root", "entities/root.xml")
        .loadout(TestLoadout::Xml("Loadouts/Root.xml".to_owned()));
    let blue = TestEntity::new(3, "Blue", "entities/blue.xml").loadout(TestLoadout::Manual(vec![
        TestLoadoutEntry::empty("mount").loadout(TestLoadout::Manual(vec![
            TestLoadoutEntry::class("capsule", "Capsule"),
        ])),
    ]));
    let database = entity_schema(DataForgeBuilder::new())
        .record(record.record())
        .record(TestEntity::new(2, "Red", "entities/red.xml").record())
        .record(blue.record())
        .record(TestEntity::new(4, "Capsule", "entities/capsule.xml").record())
        .build();
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data\\Game2.dcb", database.clone()).encrypted())
        .entry(TestEntry::stored(BASE_PATH, BASE.as_bytes()).encrypted())
        .entry(TestEntry::stored(LOCAL_PATH, LOCAL.as_bytes()))
        .write_to(&archive)?;
    let result = p4kx_testkit::p4kx()?
        .env("P4KX_ARCHIVE_KEY", p4kx_testkit::test_key().to_hex())
        .args(["--json", "entity", "loadout"])
        .arg(&archive)
        .arg("Root")
        .output()?;
    assert!(
        result.status.success(),
        "synthetic loadout resolves: {result:?}"
    );
    let json: Value = serde_json::from_slice(&result.stdout)?;
    verify_xml_names(&json)?;
    verify_xml_attachment(&json)?;
    verify_record_fields(&json, &database)?;
    verify_emitted_fields(&json)
}

fn verify_xml_names(json: &Value) -> TestResult {
    for (ordinal, name, path, xml, encrypted) in [
        (0, "left", BASE_PATH, BASE, true),
        (1, "right", LOCAL_PATH, LOCAL, false),
    ] {
        let pointer = format!("/entity/ports/{ordinal}/name");
        assert_eq!(
            json.pointer(&pointer).and_then(Value::as_str),
            Some(name),
            "{pointer}"
        );
        let receipt = origin(json, &pointer)?;
        assert_eq!(receipt["containsEncryptedSource"], encrypted, "{pointer}");
        assert_eq!(receipt["sourceSetComplete"], true, "{pointer}");
        let uses = field_uses(json, &pointer)?;
        let [original] = uses.as_slice() else {
            return Err("exactly one authored port name use".into());
        };
        assert_eq!(
            original
                .pointer("/location/namespace")
                .and_then(Value::as_str),
            Some("textXmlAttribute"),
            "the winning XML site is an original textual attribute"
        );
        assert_eq!(
            original.pointer("/location/field").and_then(Value::as_str),
            Some("portName"),
            "the exact authored name is the portName attribute"
        );
        assert_eq!(source_path(json, original)?, path, "{pointer}");
        let expected = format!("portName='{name}'");
        assert_eq!(
            original_bytes(xml.as_bytes(), original)?,
            expected.as_bytes(),
            "{pointer}"
        );
    }
    assert_eq!(
        json.pointer("/entity/ports/1/entity/class_name")
            .and_then(Value::as_str),
        Some("Blue"),
        "the original attachment target stays selected"
    );
    Ok(())
}

fn verify_xml_attachment(json: &Value) -> TestResult {
    let left = "/entity/ports/0/entity/guid";
    let right = "/entity/ports/1/entity/guid";
    for (pointer, expected) in [
        (
            left,
            &[(BASE_PATH, "itemName"), (LOCAL_PATH, "loadout")][..],
        ),
        (right, &[(LOCAL_PATH, "itemName")][..]),
    ] {
        assert_eq!(
            origin(json, pointer)?["sourceSetComplete"],
            true,
            "{pointer}"
        );
        let mut xml = Vec::new();
        for site in field_uses(json, pointer)? {
            if site["role"] == "selector"
                && site.pointer("/location/namespace").and_then(Value::as_str)
                    == Some("textXmlAttribute")
            {
                let attribute = site
                    .pointer("/location/field")
                    .and_then(Value::as_str)
                    .ok_or("original attribute")?;
                xml.push((source_path(json, site)?, attribute));
            }
        }
        xml.sort_unstable();
        assert_eq!(
            xml, expected,
            "{pointer} has the actual class and include route"
        );
    }
    Ok(())
}

fn verify_record_fields(json: &Value, database: &[u8]) -> TestResult {
    let base = "/entity/ports/1/entity";
    for (pointer, bytes) in [
        (
            format!("{base}/record_path"),
            b"entities/blue.xml".as_slice(),
        ),
        (
            format!("{base}/class_name"),
            b"EntityClassDefinition.Blue".as_slice(),
        ),
        (
            format!("{base}/ports/0/nested/0/name"),
            b"capsule".as_slice(),
        ),
    ] {
        let receipt = origin(json, &pointer)?;
        assert_eq!(receipt["containsEncryptedSource"], true, "{pointer}");
        assert_eq!(receipt["sourceSetComplete"], true, "{pointer}");
        let uses = field_uses(json, &pointer)?;
        let text = uses
            .iter()
            .copied()
            .find(|site| {
                site.pointer("/location/namespace").and_then(Value::as_str)
                    == Some("dataforgeDecodedBytes")
                    && site.pointer("/location/field").and_then(Value::as_str) == Some("text")
                    && site["role"] != "selector"
            })
            .ok_or("original DataForge text")?;
        assert_eq!(source_path(json, text)?, "Data\\Game2.dcb", "{pointer}");
        assert_eq!(original_bytes(database, text)?, bytes, "{pointer}");
    }
    let port = format!("{base}/ports/0/nested/0/name");
    let port_sources = field_uses(json, &port)?;
    assert!(
        port_sources.iter().any(|site| {
            site.pointer("/location/namespace").and_then(Value::as_str) == Some("dataforgeValue")
                && site.pointer("/location/indices/0").and_then(Value::as_str) == Some("2")
                && site["role"] == "selector"
        }),
        "the original Blue Record owns the nested override"
    );
    assert_eq!(
        origin(json, "/entry")?["containsEncryptedSource"],
        false,
        "directory metadata is separate from encrypted payload"
    );
    Ok(())
}

fn verify_emitted_fields(json: &Value) -> TestResult {
    let fields = json["fieldOrigins"].as_object().ok_or("field origins")?;
    for pointer in fields.keys() {
        assert!(
            json.pointer(pointer).is_some(),
            "{pointer} must address an emitted value"
        );
        field_uses(json, pointer)?;
    }
    Ok(())
}

fn original_bytes<'input>(
    bytes: &'input [u8],
    site: &Value,
) -> Result<&'input [u8], Box<dyn Error>> {
    let [start, end] = array(site, "/location/indices")? else {
        return Err("original byte range".into());
    };
    let index = |value: &Value| -> Result<usize, Box<dyn Error>> {
        Ok(value.as_str().ok_or("byte coordinate")?.parse()?)
    };
    bytes
        .get(index(start)?..index(end)?)
        .ok_or_else(|| "original bytes are in the authored source".into())
}

fn field_uses<'json>(
    json: &'json Value,
    pointer: &str,
) -> Result<Vec<&'json Value>, Box<dyn Error>> {
    let references = array(origin(json, pointer)?, "/sourceUsesRefs")?;
    let uses = array(json, "/provenance/uses")?;
    references
        .iter()
        .map(|id| {
            uses.iter()
                .find(|site| site["id"] == *id)
                .ok_or_else(|| format!("original use for {pointer}").into())
        })
        .collect()
}

fn source_path<'json>(json: &'json Value, use_site: &Value) -> Result<&'json str, Box<dyn Error>> {
    let reference = use_site.get("source").ok_or("physical source reference")?;
    let source = array(json, "/provenance/sources")?
        .iter()
        .find(|source| source.get("id") == Some(reference))
        .ok_or_else(|| format!("unresolved physical source: {reference}"))?;
    source
        .pointer("/path/value")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("no original archive path for {reference}").into())
}

fn origin<'json>(json: &'json Value, pointer: &str) -> Result<&'json Value, Box<dyn Error>> {
    json["fieldOrigins"]
        .get(pointer)
        .ok_or_else(|| format!("no field origin for {pointer}").into())
}

fn array<'json>(json: &'json Value, pointer: &str) -> Result<&'json [Value], Box<dyn Error>> {
    json.pointer(pointer)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| format!("no array at {pointer}").into())
}
