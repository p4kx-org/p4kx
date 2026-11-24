// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_extraction::cryxml::{SourceDocument, SourceElement, SourceReadOptions};
use p4kx_extraction::{SourceCoordinateOmission, SourceLocation};
use p4kx_testkit::{ArchiveBuilder, CryXmlByteOrder, CryXmlSignature, TestElement, TestEntry};
use serde_json::{Value, json};

use super::{TestResult, entry_path, field, original_uses, run};

const ENTRY: &str = "Data/Prefabs/test/links.xml";
const GROUPS: &str = "/objects/0/entity_links";

fn library() -> TestElement {
    let repeated = TestElement::new("Link")
        .attribute("Name", " joint ")
        .attribute("TargetId", "token")
        .attribute("Version", "0002");
    let owner = TestElement::new("Object")
        .attribute("Name", "owner")
        .attribute("Type", "Entity")
        .child(TestElement::new("Meta").child(TestElement::new("EntityLinks")))
        .child(
            TestElement::new("EntityLinks")
                .child(TestElement::new("Ignored"))
                .child(repeated.clone())
                .child(repeated)
                .child(
                    TestElement::new("Link")
                        .attribute("Name", "")
                        .attribute("TargetId", ""),
                )
                .child(TestElement::new("Link")),
        )
        .child(TestElement::new("EntityLinks"))
        .child(
            TestElement::new("EntityLinks")
                .child(TestElement::new("Link").attribute("Version", "future")),
        );
    let possible_target = TestElement::new("Object")
        .attribute("Name", "target")
        .attribute("Type", "Entity")
        .attribute("Id", "token");
    TestElement::new("PrefabsLibrary").child(
        TestElement::new("Prefab").attribute("Name", "room").child(
            TestElement::new("Objects")
                .child(owner)
                .child(possible_target),
        ),
    )
}

fn dump(bytes: &[u8]) -> Result<Value, Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let archive = dir.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(ENTRY, bytes))
        .write_to(&archive)?;
    let output = run(&["dump", "test/links", "room"], &archive, true)?;
    assert_eq!(output.code, Some(0), "{}", output.stderr);
    Ok(serde_json::from_str(&output.stdout)?)
}

fn group_source<'document>(
    source: &'document SourceDocument<'_>,
) -> impl Iterator<Item = SourceElement<'document>> {
    source
        .root()
        .elements()
        .flat_map(SourceElement::elements)
        .flat_map(SourceElement::elements)
        .take(1)
        .flat_map(SourceElement::elements)
        .filter(|child| child.name() == "EntityLinks")
}

fn assert_site(report: &Value, path: &str, location: &SourceLocation, role: &str) -> TestResult {
    let origin = field(report, path)?;
    let expected = serde_json::to_value(location)?;
    assert!(
        original_uses(report, origin)?.iter().any(|usage| {
            usage.get("location") == Some(&expected)
                && usage.get("role").and_then(Value::as_str) == Some(role)
                && usage.get("scope").and_then(Value::as_str) == Some("payload")
                && entry_path(report, usage)
                    .is_ok_and(|actual| actual.replace('\\', "/").eq_ignore_ascii_case(ENTRY))
        }),
        "missing {role} at {expected} in {ENTRY} for {path}"
    );
    Ok(())
}

fn assert_original_sites(report: &Value, bytes: &[u8]) -> TestResult {
    let source = SourceDocument::parse(bytes, SourceReadOptions::default())?;
    for (group_index, group) in group_source(&source).enumerate() {
        let group_path = format!("{GROUPS}/{group_index}");
        let group_site = SourceLocation::xml_element(&group.position(), "EntityLinks")
            .ok_or("group location")?;
        let links_path = format!("{group_path}/links");
        assert_site(report, GROUPS, &group_site, "payload")?;
        assert_site(report, &group_path, &group_site, "payload")?;
        assert_site(report, &links_path, &group_site, "payload")?;
        for (link_index, link) in group
            .elements()
            .filter(|child| child.name() == "Link")
            .enumerate()
        {
            let path = format!("{links_path}/{link_index}");
            let link_site =
                SourceLocation::xml_element(&link.position(), "Link").ok_or("link location")?;
            for pointer in [
                GROUPS,
                group_path.as_str(),
                links_path.as_str(),
                path.as_str(),
            ] {
                assert_site(report, pointer, &link_site, "payload")?;
            }
            for (field, attribute) in [
                ("name", "Name"),
                ("target_id", "TargetId"),
                ("version", "Version"),
            ] {
                let (position, role) = match link.attributes().find(|at| at.name() == attribute) {
                    Some(at) => (
                        SourceLocation::xml_attribute(at.position(), attribute),
                        "payload",
                    ),
                    None => (
                        SourceLocation::xml_element(&link.position(), attribute),
                        "conversion",
                    ),
                };
                assert_site(
                    report,
                    &format!("{path}/{field}"),
                    &position.ok_or("attribute site")?,
                    role,
                )?;
            }
        }
    }
    Ok(())
}

fn assert_collection_coverage(report: &Value) -> TestResult {
    for (path, complete) in [
        (GROUPS.to_owned(), false),
        (format!("{GROUPS}/0"), false),
        (format!("{GROUPS}/0/links"), false),
        (format!("{GROUPS}/1/links"), true),
        (format!("{GROUPS}/2/links"), true),
    ] {
        assert_eq!(
            field(report, &path)?
                .get("sourceSetComplete")
                .and_then(Value::as_bool),
            Some(complete),
            "collection coverage for {path}"
        );
    }
    Ok(())
}

fn assert_values(report: &Value) {
    let absent = json!({"name": null, "target_id": null, "version": null});
    let identical = json!({"name": " joint ", "target_id": "token", "version": "0002"});
    assert_eq!(
        report.pointer(GROUPS),
        Some(&json!([
            {"links": [identical, identical, {"name": "", "target_id": "", "version": null}, absent]},
            {"links": []},
            {"links": [{"name": null, "target_id": null, "version": "future"}]},
        ])),
        "the original group and link order, including repeated and empty declarations"
    );
    assert!(
        report.pointer("/objects/1/entity_links").is_none(),
        "an object without a direct group keeps the earlier shape"
    );
}

#[test]
fn resolved_text_links_keep_their_authored_values_and_original_byte_ranges() -> TestResult {
    let xml = format!("\u{feff}{}", library().to_xml());
    let report = dump(xml.as_bytes())?;
    assert_values(&report);
    assert_collection_coverage(&report)?;
    assert_original_sites(&report, xml.as_bytes())
}

#[test]
fn resolved_cryxml_links_keep_their_individual_original_table_coordinates() -> TestResult {
    let xml = library().to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little);
    let report = dump(&xml)?;
    assert_values(&report);
    assert_collection_coverage(&report)?;
    assert_original_sites(&report, &xml)
}

#[test]
fn a_repaired_link_value_keeps_its_physical_owner_with_opaque_coordinates() -> TestResult {
    let mut bytes = library().to_xml().into_bytes();
    let index = bytes
        .windows(b"joint".len())
        .position(|word| word == b"joint")
        .ok_or("repair target")?;
    *bytes.get_mut(index).ok_or("repair byte")? = 0xff;
    let report = dump(&bytes)?;
    let path = format!("{GROUPS}/0/links/0/name");
    assert_eq!(
        report.pointer(&path).and_then(Value::as_str),
        Some(" \u{fffd}oint "),
        "the lossy parser retains the attribute value"
    );
    assert_eq!(
        field(&report, GROUPS)?
            .get("sourceSetComplete")
            .and_then(Value::as_bool),
        Some(false),
        "a repaired collection has incomplete membership evidence"
    );
    assert_site(
        &report,
        &path,
        &SourceLocation::Opaque {
            reason: SourceCoordinateOmission::Unavailable,
        },
        "payload",
    )
}
