// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{
    DataForgeBuilder, TestEntity, TestEntry, TestLoadout, TestLoadoutEntry, entity_schema,
    sample_ship,
};
use serde_json::{Value, json};

use crate::audio_origin_assertions as proof;
use crate::support::{Install, TestResult};

const TOOL: &str = "entity_loadout";
const DCB: &str = "Data/Game2.dcb";
const ROOT: &str = "/structuredContent";
const NAME: &str = "/structuredContent/ports/0/name";
const BASE_PATH: &str = "Data/Loadouts/Base.xml";
const LOCAL_PATH: &str = "Data/Loadouts/Root.xml";
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
fn loadout_identity_receipts_follow_the_original_records_on_cached_pages() -> TestResult {
    for encrypted in [false, true] {
        let bytes = sample_ship().build();
        let entry = TestEntry::deflated(DCB, bytes.as_slice());
        let entry = if encrypted { entry.encrypted() } else { entry };
        let install = Install::new(vec![entry])?;
        let mut server = install.serve(&["--archive", "LIVE"])?;
        let tools = server.tools()?;
        let schema = tools
            .iter()
            .find(|tool| tool.get("name") == Some(&json!(TOOL)))
            .and_then(|tool| tool.get("outputSchema"))
            .ok_or("Loadout output schema")?;
        let schema = jsonschema::validator_for(schema)?;
        let mut args = json!({"entity":"TEST_Ship", "limit":1});
        let mut attached = 0;
        loop {
            let reply = server.ok(TOOL, &args)?;
            schema.validate(&reply).map_err(|error| {
                format!(
                    "loadout receipt at {} against {}: {error}",
                    error.instance_path(),
                    error.schema_path()
                )
            })?;
            proof::assert_marker(&reply, &format!("{ROOT}/archive"), false)?;
            check_record(&reply, "/entity", &bytes, encrypted)?;
            if reply.pointer("/ports/0/entity").is_some() {
                check_record(&reply, "/ports/0/entity", &bytes, encrypted)?;
                attached += 1;
            }
            let fields = reply["fieldOrigins"].as_object().ok_or("field origins")?;
            assert!(
                fields
                    .keys()
                    .all(|key| !key.starts_with(&format!("{ROOT}/ports/1"))),
                "positions are relative to this emitted page"
            );
            let Some(cursor) = reply.get("next_cursor") else {
                break;
            };
            args["cursor"] = cursor.clone();
        }
        assert!(attached > 1, "several attached Records on separate pages");
    }
    Ok(())
}

#[test]
fn loadout_port_name_receipts_preserve_winning_xml_and_nested_records_on_warm_pages() -> TestResult
{
    let root = TestEntity::new(1, "Root", "entities/root.xml")
        .loadout(TestLoadout::Xml("Loadouts/Root.xml".to_owned()));
    let red = TestEntity::new(2, "Red", "entities/red.xml");
    let blue = TestEntity::new(3, "Blue", "entities/blue.xml").loadout(TestLoadout::Manual(vec![
        TestLoadoutEntry::empty("mount").loadout(TestLoadout::Manual(vec![
            TestLoadoutEntry::class("capsule", "Red"),
        ])),
    ]));
    let database = entity_schema(DataForgeBuilder::new())
        .record(root.record())
        .record(red.record())
        .record(blue.record())
        .build();
    let install = Install::new(vec![
        TestEntry::zstd(DCB, database.clone()).encrypted(),
        TestEntry::stored(BASE_PATH, BASE.as_bytes()).encrypted(),
        TestEntry::stored(LOCAL_PATH, LOCAL.as_bytes()),
    ])?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let mut args = json!({"entity": "Root", "limit": 1});
    for (path, name, authored) in [
        ("left", "left", Some((BASE_PATH, BASE, true))),
        ("right", "right", Some((LOCAL_PATH, LOCAL, false))),
        ("right/mount", "mount", None),
        ("right/mount/capsule", "capsule", None),
    ] {
        let page = server.ok(TOOL, &args)?;
        assert_eq!(
            page.pointer("/ports/0/path"),
            Some(&json!(path)),
            "cached port path"
        );
        assert_eq!(
            page.pointer("/ports/0/name"),
            Some(&json!(name)),
            "original name"
        );
        assert_only_first_name_receipt(&page)?;
        if let Some((entry, xml, encrypted)) = authored {
            check_xml_name(&page, entry, name, xml.as_bytes(), encrypted)?;
        } else {
            check_record_name(&page, &database, name)?;
        }
        if name == "capsule" {
            assert!(
                page.get("next_cursor").is_none(),
                "the fourth port ends the walk"
            );
        } else {
            args["cursor"] = page.get("next_cursor").ok_or("next cache page")?.clone();
        }
    }
    Ok(())
}

#[test]
fn loadout_port_name_receipts_fit_with_the_rows_and_exclude_the_deferred_entry() -> TestResult {
    let left = "a".repeat(60 << 10);
    let right = "b".repeat(60 << 10);
    let directory = "loadout/".repeat(1536);
    let left_file = format!("{directory}left.xml");
    let right_file = format!("{directory}right.xml");
    let left_entry = format!("Data/{left_file}");
    let right_entry = format!("Data/{right_file}");
    let root_xml = format!(
        "<Loadout><Loadouts><Loadout loadout='{left_file}'/><Loadout loadout='{right_file}'/>\
         </Loadouts></Loadout>"
    );
    let left_xml = format!("<Loadout><Items><Item portName='{left}'/></Items></Loadout>");
    let right_xml = format!("<Loadout><Items><Item portName='{right}'/></Items></Loadout>");
    let database = entity_schema(DataForgeBuilder::new())
        .record(
            TestEntity::new(1, "Root", "entities/root.xml")
                .loadout(TestLoadout::Xml("Loadouts/Root.xml".to_owned()))
                .record(),
        )
        .build();
    let install = Install::new(vec![
        TestEntry::zstd(DCB, database),
        TestEntry::zstd(LOCAL_PATH, root_xml.into_bytes()),
        TestEntry::zstd(left_entry.as_str(), left_xml.as_bytes()),
        TestEntry::zstd(right_entry.as_str(), right_xml.as_bytes()),
    ])?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let first = server.ok(TOOL, &json!({"entity": "Root"}))?;
    let cursor = first
        .get("next_cursor")
        .and_then(Value::as_str)
        .ok_or("the origins force a second page")?;
    let second = server.ok(TOOL, &json!({"entity": "Root", "cursor": cursor}))?;
    assert_only_first_name_receipt(&first)?;
    assert_only_first_name_receipt(&second)?;
    check_xml_name(&first, &left_entry, &left, left_xml.as_bytes(), false)?;
    check_xml_name(&second, &right_entry, &right, right_xml.as_bytes(), false)?;
    assert_unseen_entry(&first, &right_entry)?;
    assert_unseen_entry(&second, &left_entry)?;
    check_fit_boundary(first, &second, left_entry.len() + right_entry.len())
}

fn check_fit_boundary(mut first: Value, second: &Value, source_path_bytes: usize) -> TestResult {
    const RESPONSE_BYTES: usize = 256 << 10;
    let bare = first.as_object_mut().ok_or("structured response")?;
    bare.remove("fieldOrigins");
    bare.remove("provenance");
    bare.remove("next_cursor");
    let row = second.pointer("/ports/0").ok_or("deferred row")?;
    bare.get_mut("ports")
        .and_then(Value::as_array_mut)
        .ok_or("first page ports")?
        .push(row.clone());
    let raw_bytes = serde_json::to_vec(&first)?.len();
    assert!(raw_bytes < RESPONSE_BYTES, "both raw rows fit together");
    assert!(
        raw_bytes + source_path_bytes > RESPONSE_BYTES,
        "the two source paths alone make the full response too large"
    );
    Ok(())
}

fn assert_only_first_name_receipt(page: &Value) -> TestResult {
    assert_eq!(
        page.pointer("/ports")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(1),
        "the fitted page has one real port"
    );
    let fields = page["fieldOrigins"].as_object().ok_or("field origins")?;
    assert!(
        fields.contains_key(NAME),
        "the emitted port has a name receipt"
    );
    assert!(
        fields
            .keys()
            .all(|key| !key.starts_with("/structuredContent/ports/1/")),
        "a deferred port has no receipt on this page"
    );
    Ok(())
}

fn assert_unseen_entry(page: &Value, unseen: &str) -> TestResult {
    let sources = page
        .pointer("/provenance/sources")
        .and_then(Value::as_array)
        .ok_or("physical sources")?;
    assert!(
        sources
            .iter()
            .all(|source| source.pointer("/path/value").and_then(Value::as_str) != Some(unseen)),
        "an unreturned port does not publish its physical source"
    );
    Ok(())
}

fn check_xml_name(
    page: &Value,
    entry: &str,
    name: &str,
    bytes: &[u8],
    encrypted: bool,
) -> TestResult {
    assert_eq!(
        page.pointer("/ports/0/name").and_then(Value::as_str),
        Some(name),
        "exact name"
    );
    proof::assert_marker(page, NAME, encrypted)?;
    let uses = proof::uses(page, NAME)?;
    let [site] = uses.as_slice() else {
        return Err("one XML payload selected the exact name".into());
    };
    assert_eq!(
        proof::source_path(page, site)?,
        entry,
        "winning XML physical source"
    );
    assert_eq!(
        site.pointer("/location/namespace"),
        Some(&json!("textXmlAttribute")),
        "same XML parser coordinate"
    );
    let attribute = format!("portName='{name}'");
    assert_eq!(
        original_bytes(bytes, site)?,
        attribute.as_bytes(),
        "exact original XML attribute"
    );
    Ok(())
}

fn check_record_name(page: &Value, bytes: &[u8], name: &str) -> TestResult {
    proof::assert_marker(page, NAME, true)?;
    let uses = proof::uses(page, NAME)?;
    let selector = uses
        .iter()
        .find(|site| site.pointer("/location/namespace") == Some(&json!("dataforgeValue")))
        .ok_or("original owned Record field")?;
    assert_eq!(
        selector.pointer("/location/indices/0"),
        Some(&json!("2")),
        "actual attached Record"
    );
    assert_eq!(
        proof::source_path(page, selector)?,
        DCB,
        "selector's physical source"
    );
    let text = uses
        .into_iter()
        .find(|site| site.pointer("/location/namespace") == Some(&json!("dataforgeDecodedBytes")))
        .ok_or("original text")?;
    assert_eq!(
        proof::source_path(page, text)?,
        DCB,
        "name's physical source"
    );
    assert_eq!(
        original_bytes(bytes, text)?,
        name.as_bytes(),
        "exact borrowed Record name bytes"
    );
    Ok(())
}

fn original_bytes<'a>(bytes: &'a [u8], site: &Value) -> TestResult<&'a [u8]> {
    let coordinate = |ordinal: usize| -> TestResult<usize> {
        Ok(site
            .pointer(&format!("/location/indices/{ordinal}"))
            .and_then(Value::as_str)
            .ok_or("original byte coordinate")?
            .parse()?)
    };
    bytes
        .get(coordinate(0)?..coordinate(1)?)
        .ok_or_else(|| "original byte range".into())
}

fn check_record(reply: &Value, entity: &str, bytes: &[u8], encrypted: bool) -> TestResult {
    for member in ["guid", "record_path", "name", "class_name"] {
        let path = format!("{ROOT}{entity}/{member}");
        proof::assert_marker(reply, &path, encrypted)?;
        for site in proof::uses(reply, &path)? {
            assert_eq!(proof::source_path(reply, site)?, DCB, "physical DCB owner");
        }
    }
    let uses = proof::uses(reply, &format!("{ROOT}{entity}/name"))?;
    let text = uses
        .into_iter()
        .find(|site| site.pointer("/location/namespace") == Some(&json!("dataforgeDecodedBytes")))
        .ok_or("original Record name bytes")?;
    let index = |ordinal: usize| -> TestResult<usize> {
        Ok(text
            .pointer(&format!("/location/indices/{ordinal}"))
            .and_then(Value::as_str)
            .ok_or("original byte coordinate")?
            .parse()?)
    };
    assert_eq!(
        bytes.get(index(0)?..index(1)?),
        reply
            .pointer(&format!("{entity}/name"))
            .and_then(Value::as_str)
            .map(str::as_bytes),
        "the attached identity points at exactly the bytes used"
    );
    Ok(())
}
