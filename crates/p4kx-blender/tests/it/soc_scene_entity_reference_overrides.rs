// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::SocContainerCapture;
use p4kx_blender::{SocScene, soc_scene_schema};
use p4kx_extraction::{Archive, CancelToken};
use p4kx_testkit::{ArchiveBuilder, TestContainer, TestEntry};
use serde_json::{Value, json};

use super::TestResult;

#[path = "soc_scene_entity_reference_overrides_fixture.rs"]
mod fixture;
use fixture::{
    authored, container, descriptor, open, package_value, placed, repeated, scene, verify_raw,
};

const CONTAINER: &str = "overrides.socpak";
const FIRST: &str = "overrides/metadata/1001/entitycomponententityreferenceoverride.xml";
const SECOND: &str = "overrides/metadata/1002/entitycomponententityreferenceoverride.xml";
const OWNER: &str = "11111111-1111-1111-1111-111111111111";
const OTHER: &str = "22222222-2222-2222-2222-222222222222";
const ROOT: &str = "EntityComponentEntityReferenceOverride";
const MEMBERS: &str = "/occurrences/0/entityReferenceOverrides";
const RAW_SOURCES: &str = "/entityReferenceOverrideSources";

#[test]
fn independent_override_documents_keep_source_order_and_share_the_scene_package_output()
-> TestResult {
    let bytes = ArchiveBuilder::new()
        .entry(descriptor())
        .entry(placed([OWNER]))
        .entry(TestEntry::stored(SECOND, authored(OWNER)).encrypted())
        .entry(TestEntry::stored(FIRST, repeated(OTHER)))
        .entry(TestEntry::stored("overrides.rmxml", b"<RoomMapping/>"))
        .build();
    let (folder, archive, inner) = open(bytes, false)?;
    let layers = [&archive, &inner];
    let capture = SocContainerCapture::read(&layers, &CancelToken::new())?;
    let scene = SocScene::from_capture(&capture)?;
    let value = serde_json::to_value(&scene)?;
    jsonschema::validator_for(&soc_scene_schema())?
        .validate(&value)
        .map_err(|error| error.to_string())?;
    let [first, repeated] = array(&value, MEMBERS)? else {
        return Err("expected both independent physical override members".into());
    };
    assert_ne!(
        at(first, "/source")?,
        at(repeated, "/source")?,
        "distinct physical sources"
    );
    assert_eq!(
        array(&value, RAW_SOURCES)?.len(),
        2,
        "only the two override payloads"
    );
    verify_primary(&value, first)?;
    verify_raw(&value, first, &authored(OWNER), Some(true))?;
    verify_repeated(&value, repeated)?;
    verify_raw(&value, repeated, &fixture::repeated(OTHER), Some(false))?;
    assert_eq!(
        at(&value, "/occurrences/0/roomCompanions/roomMapping/state")?,
        "present",
        "original room metadata coexists with the override collection"
    );
    let packed = package_value(folder.path(), &archive, &inner, &scene)?;
    assert_eq!(
        packed, value,
        "the authored-package sidecar uses the same scene payload"
    );
    Ok(())
}

#[test]
fn duplicate_paths_keep_members_and_incomplete_owner_evidence() -> TestResult {
    let payload = TestContainer::new()
        .entry(TestEntry::stored(FIRST, authored(OWNER)))
        .entry(TestEntry::stored(FIRST, repeated(OWNER)))
        .entry(TestEntry::stored(SECOND, b"<broken"))
        .entry(TestEntry::stored(
            "overrides/metadata/1003/entitycomponententityreferenceoverride.xml",
            b"<Other/>",
        ))
        .entry(
            TestEntry::stored(
                "overrides/metadata/1004/entitycomponententityreferenceoverride.xml",
                b"<Other/>",
            )
            .with_crc32(0),
        )
        .build();
    let value = scene(payload, false)?;
    let [left, right, invalid, unsupported, unreadable] = array(&value, MEMBERS)? else {
        return Err("expected the independent readable and failed physical members".into());
    };
    assert_ne!(
        at(left, "/source")?,
        at(right, "/source")?,
        "repeated paths are distinct Entries"
    );
    assert_eq!(
        at(left, "/data/entities/0/owner/state")?,
        "unreadable",
        "an incomplete census stays incomplete"
    );
    verify_raw(&value, left, &authored(OWNER), None)?;
    verify_raw(&value, right, &repeated(OWNER), None)?;
    assert_eq!(at(invalid, "/state")?, "invalid", "invalid XML is explicit");
    verify_raw(&value, invalid, b"<broken", None)?;
    verify_raw(&value, unsupported, b"<Other/>", None)?;
    let unreadable_source = at(unreadable, "/source")?;
    assert!(
        array(&value, RAW_SOURCES)?
            .iter()
            .all(|row| row.get("source") != Some(unreadable_source)),
        "CRC refusal has no verified bytes"
    );
    assert_eq!(
        at(unsupported, "/state")?,
        "unsupported",
        "other root is explicit"
    );
    assert_eq!(
        at(document(&value, unsupported, "/xml")?, "/nodes/0/name")?,
        "Other",
        "unsupported roots still retain the original raw document"
    );
    assert_eq!(
        at(unreadable, "/state")?,
        "unreadable",
        "CRC refusal is separate from malformed XML"
    );
    Ok(())
}

#[test]
fn inherited_encryption_and_ambiguous_owners_keep_all_exact_candidates() -> TestResult {
    let bytes = container([OWNER, OWNER])
        .entry(TestEntry::stored(FIRST, authored(OWNER)))
        .build();
    let value = scene(bytes, true)?;
    let member = at(&value, "/occurrences/0/entityReferenceOverrides/0")?;
    verify_raw(&value, member, &authored(OWNER), Some(true))?;
    let entity = at(member, "/data/entities/0")?;
    let owner = at(entity, "/owner")?;
    assert_eq!(
        at(owner, "/value/type")?,
        "ambiguous",
        "ambiguous owners remain explicit"
    );
    assert_eq!(
        array(owner, "/value/sites")?.len(),
        2,
        "both owning-candidate positions survive"
    );
    for path in [
        "/cryGuid/containsEncryptedSource",
        "/autoSpawnOverrides/0/elements/0/aa/containsEncryptedSource",
    ] {
        assert_eq!(
            at(entity, path)?.as_bool(),
            Some(true),
            "containing payload encryption at {path}"
        );
    }
    Ok(())
}

#[test]
fn reused_nested_members_keep_occurrences_and_never_bind_to_a_parent_owner() -> TestResult {
    let descriptor = br#"<ObjectContainer>
      <StreamingOCs><OC name="placed.soc"/></StreamingOCs>
      <ChildObjectContainers><Child name="child.socpak"/><Child name="child.socpak"/></ChildObjectContainers>
    </ObjectContainer>"#;
    let root = TestContainer::new()
        .entry(TestEntry::stored("overrides.xml", descriptor))
        .entry(placed([OWNER]))
        .build();
    let child = TestContainer::new()
        .entry(TestEntry::stored("child.xml", b"<ObjectContainer/>"))
        .entry(TestEntry::stored(FIRST, authored(OWNER)))
        .build();
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("nested.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(CONTAINER, root))
        .entry(TestEntry::stored("child.socpak", child))
        .write_to(&path)?;
    let archive = Archive::open(&path)?;
    let root = archive.entries().first().ok_or("root physical entry")?;
    let inner = archive.open_container(root)?;
    let capture = SocContainerCapture::read(&[&archive, &inner], &CancelToken::new())?;
    let value = serde_json::to_value(SocScene::from_capture(&capture)?)?;
    let [root, first, second] = array(&value, "/occurrences")? else {
        return Err("expected parent and two independently placed child occurrences".into());
    };
    assert!(
        root.get("entityReferenceOverrides").is_none(),
        "no empty optional collection"
    );
    let path = "/entityReferenceOverrides/0";
    let first_member = at(first, path)?;
    let second_member = at(second, path)?;
    assert_ne!(
        at(first, "/placement")?,
        at(second, "/placement")?,
        "placements stay distinct"
    );
    assert_eq!(
        at(first_member, "/data/xml")?,
        at(second_member, "/data/xml")?,
        "same physical document is reused"
    );
    let [raw_source] = array(&value, RAW_SOURCES)? else {
        return Err("one exact raw payload for the reused member".into());
    };
    assert_eq!(
        at(raw_source, "/source")?,
        at(first_member, "/source")?,
        "the reused raw payload is keyed by physical source"
    );
    for member in [first_member, second_member] {
        let owner = at(member, "/data/entities/0/owner")?;
        assert_eq!(
            at(owner, "/value/type")?,
            "unmatched",
            "parent SOC entity is never a child owner"
        );
    }
    Ok(())
}

fn verify_primary(scene: &Value, member: &Value) -> TestResult {
    let entity = at(member, "/data/entities/0")?;
    let entry = at(entity, "/autoSpawnOverrides/0/elements/0")?;
    assert_eq!(
        at(entity, "/ordinal")?,
        1,
        "entity keeps its root sibling position"
    );
    assert_eq!(
        at(entity, "/autoSpawnOverrides/0/ordinal")?,
        1,
        "group keeps its sibling position"
    );
    assert_eq!(
        at(entry, "/ordinal")?,
        1,
        "override keeps its sibling position"
    );
    assert_eq!(
        at(entity, "/owner/value/type")?,
        "matched",
        "complete local SOC census resolves owner"
    );
    let raw = document(scene, member, "/data/xml")?;
    for (carrier, name, lexeme) in [
        (at(entity, "/cryGuid")?, "CryGUID", OWNER),
        (at(entry, "/autoSpawnIdx")?, "AutoSpawnIdx", "0007"),
        (at(entry, "/target")?, "Target", "raw,, chain"),
        (at(entry, "/aa")?, "AA", OTHER),
    ] {
        assert_eq!(at(carrier, "/value")?, lexeme, "exact authored {name}");
        assert_eq!(
            at(carrier, "/containsEncryptedSource")?.as_bool(),
            Some(true),
            "physical member contributes {name}"
        );
        let attribute = attribute(raw, name)?;
        assert_eq!(at(attribute, "/value")?, lexeme, "raw XML keeps {name}");
        verify_origin(scene, carrier, at(member, "/source")?, attribute, name)?;
    }
    Ok(())
}

fn verify_repeated(scene: &Value, member: &Value) -> TestResult {
    let entity = at(member, "/data/entities/0")?;
    let group = at(entity, "/autoSpawnOverrides/0")?;
    let entry = at(group, "/elements/0")?;
    assert_eq!(
        at(entity, "/owner/value/type")?,
        "unmatched",
        "other entity is unresolved"
    );
    assert_eq!(
        at(entry, "/autoSpawnIdx/state")?,
        "absent",
        "missing attribute is absent"
    );
    assert_eq!(at(entry, "/target/value")?, "", "empty string is present");
    assert_eq!(
        at(entry, "/aa/state")?,
        "invalid",
        "repeated values have no unique typed value"
    );
    assert_eq!(
        at(entry, "/aa/containsEncryptedSource")?.as_bool(),
        Some(false),
        "unrelated encrypted companion does not attribute this value"
    );
    let raw = document(scene, member, "/data/xml")?;
    let attrs = array(raw, "/nodes/3/attributes")?;
    let values: Vec<_> = attrs
        .iter()
        .filter_map(|attribute| attribute.get("value").and_then(Value::as_str))
        .collect();
    assert_eq!(
        values,
        ["", "first", "second", "retained"],
        "raw repeated and additional attributes stay in order"
    );
    assert!(
        array(entity, "/autoSpawnOverrides/1/elements")?.is_empty(),
        "empty group is retained"
    );
    Ok(())
}

fn verify_origin(
    scene: &Value,
    carrier: &Value,
    source: &Value,
    attribute: &Value,
    name: &str,
) -> TestResult {
    let position = at(attribute, "/position")?;
    let indices = json!([
        at(position, "/node")?
            .as_u64()
            .ok_or("CryXml node")?
            .to_string(),
        at(position, "/attribute")?
            .as_u64()
            .ok_or("CryXml attribute")?
            .to_string(),
    ]);
    let refs = array(carrier, "/sourceUsesRefs")?;
    assert!(
        array(scene, "/sourceInventory/uses")?.iter().any(|used| {
            used.get("id").is_some_and(|id| refs.contains(id))
                && used.get("source") == Some(source)
                && used.pointer("/location/field").and_then(Value::as_str) == Some(name)
                && used.pointer("/location/indices") == Some(&indices)
                && used.get("role").and_then(Value::as_str) == Some("payload")
        }),
        "exact CryXml coordinates and payload identity for {name}"
    );
    Ok(())
}

fn document<'a>(scene: &'a Value, member: &Value, path: &str) -> TestResult<&'a Value> {
    let index = at(member, path)?.as_u64().ok_or("physical XML document")?;
    at(scene, &format!("/documents/{index}"))
}

fn attribute<'a>(xml: &'a Value, name: &str) -> TestResult<&'a Value> {
    array(xml, "/nodes")?
        .iter()
        .filter_map(|node| node.get("attributes").and_then(Value::as_array))
        .flatten()
        .find(|attribute| attribute.get("name").and_then(Value::as_str) == Some(name))
        .ok_or_else(|| format!("missing raw attribute {name}").into())
}

fn array<'a>(value: &'a Value, path: &str) -> TestResult<&'a [Value]> {
    at(value, path)?
        .as_array()
        .map(Vec::as_slice)
        .ok_or_else(|| format!("expected array at {path}").into())
}

fn at<'a>(value: &'a Value, path: &str) -> TestResult<&'a Value> {
    value
        .pointer(path)
        .ok_or_else(|| format!("expected value at {path}").into())
}
