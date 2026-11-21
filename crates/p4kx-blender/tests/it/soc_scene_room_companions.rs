// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::SocContainerCapture;
use p4kx_blender::{SocScene, soc_scene_schema};
use p4kx_extraction::{Archive, CancelToken};
use p4kx_testkit::{
    ArchiveBuilder, CryXmlByteOrder, CryXmlSignature, TestChunkFile, TestContainer, TestElement,
    TestEntry,
};
use serde_json::{Value, json};

use super::TestResult;

const CONTAINER: &str = "rooms.socpak";
const MAPPING: &str = "rooms.rmxml";
const CONNECTORS: &str = "rooms/metadata/entitycomponentroomconnector.xml";
const OWNER: &str = "11111111-1111-1111-1111-111111111111";
const OTHER: &str = "22222222-2222-2222-2222-222222222222";
const OCCURRENCE: &str = "/occurrences/0";
const COMPANIONS: &str = "/occurrences/0/roomCompanions";

#[test]
fn verified_room_mapping_and_connector_members_keep_their_original_order_and_coordinates()
-> TestResult {
    let value = scene(
        container(common([OWNER]))
            .entry(TestEntry::stored(MAPPING, mapping()))
            .entry(TestEntry::stored(
                CONNECTORS,
                connectors([OWNER, OTHER, OWNER]),
            ))
            .build(),
    )?;
    jsonschema::validator_for(&soc_scene_schema())?
        .validate(&value)
        .map_err(|error| error.to_string())?;
    let occurrence = value_at(&value, OCCURRENCE)?;
    let mapping = value_at(occurrence, "/roomCompanions/roomMapping")?;
    let connectors = value_at(occurrence, "/roomCompanions/roomConnectors")?;
    assert_text_at(mapping, "/state", "present")?;
    assert_text_at(connectors, "/state", "present")?;
    assert_ne!(
        value_at(mapping, "/source")?,
        value_at(connectors, "/source")?,
        "each member has its own physical identity"
    );
    verify_mapping(document(&value, value_at(mapping, "/data")?)?)?;
    let connector_xml = document(&value, value_at(connectors, "/data/xml")?)?;
    assert_eq!(
        value_at(connector_xml, "/nodes/1/attributes/4/value")?.as_str(),
        Some("0.999,0,-0,0"),
        "raw quaternion preserves the original signed zero spelling"
    );
    verify_resolved_connectors(connectors)?;
    assert_text_at(occurrence, "/registry/state", "present")?;
    assert_text_at(occurrence, "/streams/0/member/state", "present")
}

#[test]
fn independently_encrypted_companion_and_target_keep_distinct_origin_domains() -> TestResult {
    let [descriptor, registry, stream] = common([OWNER]);
    let bytes = ArchiveBuilder::new()
        .entry(descriptor)
        .entry(registry)
        .entry(stream.encrypted())
        .entry(TestEntry::stored(MAPPING, mapping()).encrypted())
        .entry(TestEntry::stored(CONNECTORS, connectors([OWNER])))
        .build();
    let value = scene(bytes)?;
    let members = value_at(&value, COMPANIONS)?;
    let mapping = document(&value, value_at(members, "/roomMapping/data")?)?;
    assert_eq!(
        value_at(mapping, "/nodes/1/attributes/0/containsEncryptedSource")?.as_bool(),
        Some(true),
        "an encrypted physical mapping member marks its own attributes"
    );
    let connectors = value_at(members, "/roomConnectors/data")?;
    let row = value_at(connectors, "/connectors/0")?;
    assert_eq!(
        value_at(row, "/crossSectionPosition/containsEncryptedSource")?.as_bool(),
        Some(false),
        "a plain companion coordinate does not inherit the unrelated mapping or owner encryption"
    );
    verify_owner_evidence(&value, value_at(row, "/owner")?)?;
    let coordinate = document(&value, value_at(connectors, "/xml")?)?;
    assert_text_at(coordinate, "/nodes/1/attributes/4/position/type", "cryXml")
}

#[test]
fn room_companion_selection_distinguishes_ambiguity_invalid_unsupported_and_unreadable()
-> TestResult {
    let value = scene(
        container(common([OWNER, OWNER]))
            .entry(TestEntry::stored(MAPPING, mapping()))
            .entry(TestEntry::stored("ROOMS.RMXML", mapping()))
            .entry(TestEntry::stored(CONNECTORS, connectors([OWNER])))
            .build(),
    )?;
    let members = value_at(&value, COMPANIONS)?;
    let mapping = value_at(members, "/roomMapping")?;
    assert_text_at(mapping, "/state", "ambiguous")?;
    let [first, second] = array_at(mapping, "/candidates")? else {
        return Err("expected both physical mapping candidates".into());
    };
    assert_ne!(first, second, "duplicates have independent identities");
    let owner = value_at(members, "/roomConnectors/data/connectors/0/owner/value")?;
    assert_text_at(owner, "/type", "ambiguous")?;
    assert_eq!(
        array_at(owner, "/sites")?.len(),
        2,
        "both exact SOC owner candidates are retained"
    );
    verify_unreadable_members()
}

#[test]
fn incomplete_soc_census_leaves_the_connector_owner_unverified() -> TestResult {
    let value = scene(
        TestContainer::new()
            .entry(TestEntry::stored(CONNECTORS, connectors([OWNER])))
            .build(),
    )?;
    let members = value_at(&value, COMPANIONS)?;
    assert_text_at(members, "/roomMapping/state", "missing")?;
    let owner = value_at(members, "/roomConnectors/data/connectors/0/owner")?;
    assert_text_at(owner, "/state", "unreadable")?;
    assert_eq!(
        value_at(owner, "/sourceSetComplete")?.as_bool(),
        Some(false),
        "incomplete owner evidence is visible"
    );
    Ok(())
}

fn verify_owner_evidence(value: &Value, owner: &Value) -> TestResult {
    assert_eq!(
        value_at(owner, "/containsEncryptedSource")?.as_bool(),
        Some(true),
        "the owner link incorporates the encrypted physical SOC stream"
    );
    assert_eq!(
        value_at(owner, "/sourceSetComplete")?.as_bool(),
        Some(true),
        "the unique same-container owner has complete evidence"
    );
    let refs = array_at(owner, "/sourceUsesRefs")?;
    let mut roles = Vec::new();
    for used in array_at(value, "/sourceInventory/uses")? {
        if refs.contains(value_at(used, "/id")?) {
            roles.push(
                value_at(used, "/role")?
                    .as_str()
                    .ok_or("source use role must be text")?,
            );
        }
    }
    assert!(
        roles.contains(&"selector") && roles.contains(&"payload"),
        "the owner link carries both connector and SOC entity evidence"
    );
    Ok(())
}

fn verify_unreadable_members() -> TestResult {
    let unreadable = scene(
        container(common([OWNER]))
            .entry(TestEntry::stored(MAPPING, b"<RoomMapping").with_crc32(0))
            .entry(TestEntry::stored(
                CONNECTORS,
                b"<EntityComponentRoomConnector",
            ))
            .build(),
    )?;
    let members = value_at(&unreadable, COMPANIONS)?;
    assert_text_at(members, "/roomMapping/state", "unreadable")?;
    assert_text_at(members, "/roomConnectors/state", "invalid")?;
    let unsupported = scene(
        container(common([OWNER]))
            .entry(TestEntry::stored(CONNECTORS, b"<Other/>"))
            .build(),
    )?;
    let member = value_at(value_at(&unsupported, COMPANIONS)?, "/roomConnectors")?;
    assert_text_at(member, "/state", "unsupported")?;
    let xml = document(&unsupported, value_at(member, "/xml")?)?;
    assert_text_at(xml, "/nodes/0/name", "Other")
}

fn verify_resolved_connectors(connectors: &Value) -> TestResult {
    let [first, unmatched, repeated] = array_at(connectors, "/data/connectors")? else {
        return Err("expected three original connector declarations".into());
    };
    assert_text_at(unmatched, "/owner/value/type", "unmatched")?;
    assert_eq!(
        value_at(repeated, "/ordinal")?.as_u64(),
        Some(2),
        "a repeated owner keeps its original position"
    );
    assert_text_at(repeated, "/cryGuid/value", OWNER)?;
    assert_eq!(
        value_at(first, "/crossSectionRotation/value")?,
        &json!([0.999, 0.0, -0.0, 0.0]),
        "the quaternion stays in stored order"
    );
    assert_text_at(first, "/owner/value/type", "matched")?;
    assert_eq!(
        value_at(first, "/owner/value/site/ordinal")?.as_u64(),
        Some(0),
        "the owner link targets the exact original Entity ordinal"
    );
    Ok(())
}

fn verify_mapping(xml: &Value) -> TestResult {
    assert_eq!(
        array_at(xml, "/nodes")?.len(),
        5,
        "all original nodes are present"
    );
    assert_eq!(
        value_at(xml, "/nodes/2/parent")?.as_u64(),
        Some(1),
        "the original ordered nesting remains addressable"
    );
    for (path, original) in [
        ("/nodes/1/attributes/0/value", ""),
        ("/nodes/2/attributes/0/value", OWNER),
        ("/nodes/3/attributes/0/value", OWNER),
        ("/nodes/2/attributes/1/value", "3.40e+38"),
        ("/nodes/2/attributes/2/value", "0"),
    ] {
        assert_text_at(xml, path, original)?;
    }
    assert_text_at(
        xml,
        "/nodes/4/attributes/0/value",
        &format!("{OWNER},{OTHER}"),
    )
}

fn assert_text_at(value: &Value, path: &str, expected: &str) -> TestResult {
    assert_eq!(
        value_at(value, path)?.as_str(),
        Some(expected),
        "unexpected text at {path}"
    );
    Ok(())
}

fn array_at<'a>(value: &'a Value, path: &str) -> TestResult<&'a [Value]> {
    value_at(value, path)?
        .as_array()
        .map(Vec::as_slice)
        .ok_or_else(|| format!("expected an array at {path}").into())
}

fn value_at<'a>(value: &'a Value, path: &str) -> TestResult<&'a Value> {
    value
        .pointer(path)
        .ok_or_else(|| format!("expected a value at {path}").into())
}

fn document<'a>(value: &'a Value, index: &Value) -> TestResult<&'a Value> {
    let index = index.as_u64().ok_or("original XML document index")?;
    value_at(value, &format!("/documents/{index}"))
}

fn mapping() -> String {
    format!(
        r#"<RoomMapping><_ SuperGUID=""><_ SuperGUID="{OWNER}" Distance="3.40e+38" Features="0"/><_ SuperGUID="{OWNER}"/></_><_ SuperGUID="{OWNER},{OTHER}"/></RoomMapping>"#
    )
}

fn connectors<const N: usize>(owners: [&str; N]) -> Vec<u8> {
    owners
        .into_iter()
        .fold(
            TestElement::new("EntityComponentRoomConnector"),
            |root, owner| {
                root.child(
                    TestElement::new("Entity")
                        .attribute("CryGUID", owner)
                        .attribute("PivotRelativePos", "1,2,3")
                        .attribute("PivotRelativeRot", "1,0,0,0")
                        .attribute("CrossSectionPos", "4,5,6")
                        .attribute("CrossSectionRot", "0.999,0,-0,0"),
                )
            },
        )
        .to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little)
}

fn common<const N: usize>(owners: [&str; N]) -> [TestEntry; 3] {
    let entities = owners
        .into_iter()
        .fold(TestElement::new("Entities"), |root, owner| {
            root.child(TestElement::new("Entity").attribute("EntityCryGUID", owner))
        });
    let stream = TestChunkFile::crch()
        .chunk(
            4,
            3,
            entities.to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little),
        )
        .bytes();
    [
        TestEntry::stored("rooms.xml", br#"<ObjectContainer><StreamingOCs><OC name="placed.soc"/></StreamingOCs></ObjectContainer>"#),
        TestEntry::stored("rooms/metadata/entitycomponentroom.xml",
            format!(r#"<EntityComponentRoom><Entity CryGUID="{OWNER}"/></EntityComponentRoom>"#)),
        TestEntry::stored("placed.soc", stream),
    ]
}

fn container(entries: [TestEntry; 3]) -> TestContainer {
    entries
        .into_iter()
        .fold(TestContainer::new(), TestContainer::entry)
}

fn scene(bytes: Vec<u8>) -> TestResult<Value> {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("scene.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(CONTAINER, bytes))
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
    let entry = archive
        .entries()
        .first()
        .ok_or("the original container entry")?;
    let inner = archive.open_container(entry)?;
    let cancel = CancelToken::new();
    let capture = SocContainerCapture::read(&[&archive, &inner], &cancel)?;
    Ok(serde_json::to_value(SocScene::from_capture_with_cancel(
        &capture, &cancel,
    )?)?)
}
