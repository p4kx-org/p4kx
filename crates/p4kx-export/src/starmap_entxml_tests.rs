// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceLocation, SourceUseRole, SourceUses};
use p4kx_testkit::{CryXmlByteOrder, CryXmlSignature, TestContainer, TestElement, TestEntry};
use serde_json::{Value, json};

use super::{FOLDER, Limits, external, open, read, root, text_of};
use crate::catalog_read::original::with_entry_capture;
use crate::export_ledger::SourceDisposition;
use crate::starmap_containers::{PlacementScope, Placements};

#[test]
fn repeated_locations_select_one_original_entity_with_independent_field_sources() {
    let dir = tempfile::tempdir().expect("a directory");
    let guid = text_of(24);
    let class = text_of(25);
    let sub_config = text_of(26);
    let invalid_sub_config = "broken selector";
    let area_guid = text_of(27);
    let entity_path = "nested/EnTdAtA/67.EntXML";
    let descriptor = root(vec![location(&guid, &class), location(&guid, &class)]);
    let entity =
        root_with_harvestable_and_area(&guid, &class, &sub_config, invalid_sub_config, &area_guid);
    let archive = open(
        dir.path(),
        vec![container(
            "tsystem",
            &descriptor,
            [member(entity_path, &entity)],
        )],
    );
    let (mut placements, sources) = with_entry_capture(&archive, |capture| {
        let placements =
            Placements::read(&archive, None, capture, PlacementScope::RecordSelection)?;
        Ok((placements, capture.source_uses().clone()))
    })
    .expect("sourced placements");
    let rows = placement_rows(&mut placements);
    let [first, second] = rows.as_slice() else {
        panic!("two authored locations must be exported");
    };
    assert_ne!(first["instanceId"]["value"], second["instanceId"]["value"]);
    let first = entxml(first);
    let second = entxml(second);
    assert_eq!(first["inventoryComplete"], true);
    assert_eq!(first["selected"]["state"], "present");
    assert_eq!(
        first["selected"]["value"],
        first["candidates"]["value"][0]["entry"]["value"]
    );
    assert_ne!(
        first["selected"]["sourceUsesRefs"],
        second["selected"]["sourceUsesRefs"]
    );
    let body = &first["candidates"]["value"][0]["document"]["value"];
    assert_eq!(body, &second["candidates"]["value"][0]["document"]["value"]);
    assert_eq!(body["rootGuid"]["value"], guid);
    assert_eq!(body["classGuid"]["value"], class);
    let valid = assert_component_occurrences(body, &guid, &sub_config, invalid_sub_config);
    assert_area_occurrences(body, &area_guid);
    assert_entity_sources(&sources, &first["selected"], valid, entity_path);
    assert_one_projected_member(&placements, entity_path);
}

fn assert_one_projected_member(placements: &Placements, entity_path: &str) {
    assert_eq!(
        placements
            .ledger
            .entries
            .iter()
            .filter(|entry| {
                entry
                    .path
                    .as_str()
                    .replace('\\', "/")
                    .ends_with(entity_path)
            })
            .count(),
        1,
        "a physically reused identity is read once within the survey"
    );
}

fn root_with_harvestable_and_area(
    guid: &str,
    class: &str,
    sub_config: &str,
    invalid_sub_config: &str,
    area_guid: &str,
) -> TestElement {
    entity(guid, class)
        .child(harvestable(sub_config))
        .child(
            TestElement::new("AreaSphereComponent").child(
                TestElement::new("Entities")
                    .child(entity_ref(area_guid))
                    .child(entity_ref(area_guid)),
            ),
        )
        .child(harvestable(invalid_sub_config))
}

fn assert_component_occurrences<'row>(
    body: &'row Value,
    guid: &str,
    sub_config: &str,
    invalid_sub_config: &str,
) -> &'row Value {
    let components = body["harvestableComponents"]["value"]
        .as_array()
        .expect("original components");
    let [valid, invalid] = components.as_slice() else {
        panic!("both root component occurrences must remain in source order");
    };
    assert_eq!(valid["ownerGuid"]["value"], guid);
    assert_eq!(invalid["ownerGuid"]["value"], guid);
    assert_eq!(valid["subConfigGuid"]["value"], sub_config);
    assert_eq!(invalid["subConfigGuid"]["state"], "invalid");
    assert_eq!(invalid["subConfigText"]["value"], invalid_sub_config);
    assert_eq!(valid["nodeIndex"]["value"], 1);
    assert_eq!(invalid["nodeIndex"]["value"], 6);
    valid
}

fn assert_area_occurrences(body: &Value, area_guid: &str) {
    let areas = &body["areaEntities"]["value"];
    assert_eq!(areas[0]["guid"]["value"], area_guid);
    assert_eq!(areas[1]["guid"]["value"], area_guid);
    assert_eq!(areas[0]["sphereNodeIndex"]["value"], 2);
    assert_eq!(areas[1]["sphereNodeIndex"]["value"], 2);
    assert_eq!(areas[0]["nodeIndex"]["value"], 4);
    assert_eq!(areas[1]["nodeIndex"]["value"], 5);
}

fn assert_entity_sources(sources: &SourceUses, selected: &Value, valid: &Value, path: &str) {
    assert_field_sources(
        sources,
        selected,
        &[
            ("tsystem.xml", "@guid", SourceUseRole::Selector),
            (path, "@EntityCryGUID", SourceUseRole::Selector),
        ],
    );
    assert_field_sources(
        sources,
        &valid["subConfigGuid"],
        &[(path, "@SubConfigGUID", SourceUseRole::Conversion)],
    );
    assert_field_sources(
        sources,
        &valid["ownerGuid"],
        &[(path, "@EntityCryGUID", SourceUseRole::Conversion)],
    );
}

#[test]
fn filename_class_and_other_container_are_ineligible_matches() {
    let dir = tempfile::tempdir().expect("a directory");
    let guid = text_of(31);
    let class = text_of(32);
    let other = text_of(33);
    let descriptor = root(vec![location(&guid, &class), external("secondary")]);
    let irrelevant = root(vec![location(&other, &class)]);
    let mut placements = read(
        dir.path(),
        vec![
            container(
                "tsystem",
                &descriptor,
                [
                    member(
                        &format!("nested/entdata/{guid}.entxml"),
                        &entity(&other, &class),
                    ),
                    member("not_entdata/1.entxml", &entity(&guid, &class)),
                ],
            ),
            container(
                "secondary",
                &irrelevant,
                [member("entdata/1.entxml", &entity(&guid, &class))],
            ),
        ],
        Limits::default(),
    );
    for row in placement_rows(&mut placements) {
        let binding = entxml(&row);
        assert_eq!(binding["inventoryComplete"], true);
        assert_eq!(binding["selected"]["state"], "absent");
        assert_eq!(binding["candidates"]["value"], json!([]));
    }
    assert!(
        placements
            .ledger
            .entries
            .iter()
            .all(|entry| !entry.path.as_str().contains("not_entdata")),
        "a substring is not an entdata path component"
    );
}

#[test]
fn duplicate_entities_keep_every_observed_document_in_physical_order() {
    let dir = tempfile::tempdir().expect("a directory");
    let guid = text_of(41);
    let class = text_of(42);
    let first = text_of(43);
    let second = text_of(44);
    let descriptor = root(vec![location(&guid, &class)]);
    let mut placements = read(
        dir.path(),
        vec![container(
            "tsystem",
            &descriptor,
            [
                member(
                    "entdata/z.entxml",
                    &entity(&guid, &class).child(harvestable(&first)),
                ),
                member(
                    "entdata/a.entxml",
                    &entity(&guid, &class).child(harvestable(&second)),
                ),
            ],
        )],
        Limits::default(),
    );
    let rows = placement_rows(&mut placements);
    let binding = entxml(&rows[0]);
    assert_eq!(binding["inventoryComplete"], true);
    assert_eq!(binding["selected"]["state"], "invalid");
    let candidates = binding["candidates"]["value"]
        .as_array()
        .expect("candidate array");
    let [actual_first, actual_second] = candidates.as_slice() else {
        panic!("both physically admitted documents must remain visible");
    };
    assert_ne!(
        actual_first["entry"]["value"],
        actual_second["entry"]["value"]
    );
    let sub_config = |candidate: &Value| {
        candidate["document"]["value"]["harvestableComponents"]["value"][0]["subConfigGuid"]["value"].clone()
    };
    assert_eq!(sub_config(actual_first), first);
    assert_eq!(sub_config(actual_second), second);
}

#[test]
fn a_failed_sibling_prevents_uniqueness_without_erasing_the_read_entity() {
    let dir = tempfile::tempdir().expect("a directory");
    let guid = text_of(51);
    let class = text_of(52);
    let sub_config = text_of(53);
    let other = text_of(54);
    let broken_path = "nested/entdata/15.entxml";
    let descriptor = root(vec![location(&guid, &class), location(&other, &class)]);
    let mut placements = read(
        dir.path(),
        vec![container(
            "tsystem",
            &descriptor,
            [
                member(
                    "entdata/16.entxml",
                    &entity(&guid, &class).child(harvestable(&sub_config)),
                ),
                member(broken_path, &entity(&other, &class)).with_crc32(0),
            ],
        )],
        Limits::default(),
    );
    let rows = placement_rows(&mut placements);
    let [known, unknown] = rows.as_slice() else {
        panic!("the ledger failure does not erase authored placement rows");
    };
    for row in [known, unknown] {
        let binding = entxml(row);
        assert_eq!(binding["inventoryComplete"], false);
        assert_eq!(binding["selected"]["state"], "unreadable");
        assert_eq!(binding["selected"]["sourceSetComplete"], false);
    }
    let known_candidates = entxml(known)["candidates"]["value"]
        .as_array()
        .expect("observed candidates");
    let [known_candidate] = known_candidates.as_slice() else {
        panic!("preserve the actual readable candidate");
    };
    assert_eq!(known_candidate["document"]["state"], "present");
    assert_eq!(
        known_candidate["document"]["value"]["harvestableComponents"]["value"][0]["subConfigGuid"]
            ["value"],
        sub_config
    );
    assert_eq!(entxml(unknown)["candidates"]["value"], json!([]));
    assert!(placements.ledger.entries.iter().any(|entry| {
        entry.disposition == SourceDisposition::ReadFailed
            && entry
                .path
                .as_str()
                .replace('\\', "/")
                .ends_with(broken_path)
    }));
}

fn assert_field_sources(uses: &SourceUses, cell: &Value, expected: &[(&str, &str, SourceUseRole)]) {
    let refs = cell["sourceUsesRefs"].as_array().expect("field origins");
    let inventory = uses.freeze().expect("original source inventory");
    for &(expected_path, expected_field, expected_role) in expected {
        assert!(
            refs.iter().any(|id| {
                let id = id.as_str().expect("source-use reference");
                let usage = inventory
                    .uses()
                    .iter()
                    .find(|row| row.id().as_str() == id)
                    .expect("source-use reference resolves");
                let SourceLocation::Subresource {
                    namespace, field, ..
                } = usage.location()
                else {
                    return false;
                };
                let original = inventory
                    .sources()
                    .iter()
                    .find(|row| row.id() == usage.source())
                    .expect("physical reference resolves");
                let original = serde_json::to_value(original).expect("physical source row");
                namespace == "xml"
                    && field == expected_field
                    && usage.role() == expected_role
                    && original["path"]["value"]
                        .as_str()
                        .is_some_and(|path| path.replace('\\', "/").ends_with(expected_path))
            }),
            "missing {expected_field} {expected_role:?} in {expected_path}"
        );
    }
}

fn location(guid: &str, class: &str) -> TestElement {
    TestElement::new("Child")
        .attribute("locationType", "3")
        .attribute("guid", guid)
        .attribute("classGuid", class)
}

fn entity(guid: &str, class: &str) -> TestElement {
    entity_ref(guid).attribute("EntityClassGUID", class)
}

fn entity_ref(guid: &str) -> TestElement {
    TestElement::new("Entity").attribute("EntityCryGUID", guid)
}

fn harvestable(sub_config_guid: &str) -> TestElement {
    TestElement::new("HarvestableComponent").attribute("SubConfigGUID", sub_config_guid)
}

fn member(path: &str, entity: &TestElement) -> TestEntry {
    TestEntry::stored(
        path,
        entity.to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little),
    )
}

fn container(name: &str, xml: &str, members: impl IntoIterator<Item = TestEntry>) -> TestEntry {
    let archive =
        TestContainer::new().entry(TestEntry::stored(format!("{name}.xml"), xml.as_bytes()));
    let archive = members.into_iter().fold(archive, TestContainer::entry);
    TestEntry::stored(format!("{FOLDER}{name}.socpak"), archive.build())
}

fn placement_rows(placements: &mut Placements) -> Vec<Value> {
    placements
        .take_locations()
        .rows
        .iter()
        .map(|row| serde_json::to_value(row).expect("location row"))
        .collect()
}

fn entxml(row: &Value) -> &Value {
    &row["declaration"]["entXml"]
}
