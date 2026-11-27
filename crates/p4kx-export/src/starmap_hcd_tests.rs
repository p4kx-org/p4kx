// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourceUses;
use p4kx_testkit::{TestContainer, TestElement, TestEntry};
use serde_json::Value;

use super::{FOLDER, Limits, child, external, open, placed, read, root, socpak, text_of};
use crate::catalog_read::original::with_entry_capture;
use crate::export_entry_ledger::EntryOperation;
use crate::export_ledger::SourceDisposition;
use crate::starmap_containers::location_instance::LocationInstance;
use crate::starmap_containers::{PlacementScope, Placements};

#[test]
fn a_container_with_only_hcd_xml_has_no_placement_body() {
    let dir = tempfile::tempdir().expect("a directory");
    let path = format!("HCD/{}.xml", text_of(17));
    let xml = root(vec![child("9,8,7", 1)]);
    let system = TestEntry::stored(
        format!("{FOLDER}tsystem.socpak"),
        TestContainer::new()
            .entry(TestEntry::stored(&path, xml.as_bytes()))
            .entry(TestEntry::stored("empty.soc", Vec::new()))
            .build(),
    );
    let mut placements = read(dir.path(), vec![system], Limits::default());
    assert_eq!(
        placed(&placements, 1),
        None,
        "the HCD Child is not a placement input"
    );
    assert_eq!(placements.counts.unreadable, 1, "the main XML is missing");
    assert_eq!(
        placements.take_instances().rows.len(),
        0,
        "a missing placement body does not create a container occurrence"
    );
    assert!(
        placements.ledger.entries.iter().any(|entry| {
            entry.operation == EntryOperation::ContainerXml
                && entry.disposition == SourceDisposition::ProjectionFailed
                && entry.container_entry_index.is_none()
        }),
        "the failure belongs to the actual opened outer container"
    );
    assert!(
        placements
            .ledger
            .entries
            .iter()
            .all(|entry| !entry.path.as_str().ends_with(&path)),
        "the unrelated HCD Entry is not attempted"
    );
}

#[test]
fn an_hcd_basename_cannot_displace_the_eligible_fallback_xml() {
    let dir = tempfile::tempdir().expect("a directory");
    let tile = text_of(23);
    let hcd = format!("body/subfolder/hCd\\{tile}.xml");
    let decoy = root(vec![child("9,8,7", 1)]);
    let actual = root(vec![child("1,2,3", 2)]);
    let tile_entry = TestEntry::stored(
        format!("{FOLDER}{tile}.socpak"),
        TestContainer::new()
            .entry(TestEntry::stored(&hcd, decoy.as_bytes()))
            .entry(TestEntry::stored("actual.xml", actual.as_bytes()))
            .build(),
    );
    let system = socpak("tsystem", &root(vec![external(&tile)]));
    let placements = read(dir.path(), vec![system, tile_entry], Limits::default());
    assert_eq!(
        placed(&placements, 1),
        None,
        "the matching HCD basename is ineligible"
    );
    assert_eq!(
        placed(&placements, 2),
        Some([1.0, 2.0, 3.0]),
        "the eligible legacy XML still supplies the placement"
    );
    assert!(
        placements.ledger.entries.iter().any(|entry| {
            entry.operation == EntryOperation::ContainerXml
                && entry.disposition == SourceDisposition::Projected
                && entry.path.as_str().ends_with("!actual.xml")
        }),
        "the actual fallback Entry owns the successful read"
    );
    assert!(
        placements
            .ledger
            .entries
            .iter()
            .all(|entry| !entry.path.as_str().ends_with(&hcd)),
        "the unrelated HCD Entry does not acquire a payload receipt"
    );
}

#[test]
fn a_location_reads_its_same_container_hcd_once_with_its_own_field_sources() {
    let dir = tempfile::tempdir().expect("a directory");
    let guid = text_of(23);
    let preset = text_of(24);
    let xml = root(vec![location(&guid, "1"), location(&guid, "true")]);
    let file = format!("nested/HcD/{guid}.xml");
    let wrong = format!("{guid}.xml");
    let document = format!(
        r#"<HierarchicalContextData __version="2" preset="{preset}" presetRecordName="Synthetic" extra="keep"/>"#
    );
    let inner = [
        TestEntry::stored(&wrong, b"<Decoy/>".as_slice()),
        TestEntry::stored(&file, document.as_bytes()),
    ];
    let archive = open(dir.path(), vec![root_container(&xml, inner)]);
    let (mut placements, uses) = with_entry_capture(&archive, |capture| {
        let placements =
            Placements::read(&archive, None, capture, PlacementScope::RecordSelection)?;
        Ok((placements, capture.source_uses().clone()))
    })
    .expect("catalog placement input");
    let rows = placements.take_locations();
    let [first, second] = rows.rows.as_slice() else {
        panic!("both original Child occurrences need a row");
    };
    let first = context(first);
    let second = context(second);
    assert_eq!(
        first["document"]["state"], "present",
        "read the declared companion"
    );
    assert_eq!(
        first["document"]["value"]["presetGuid"]["value"], preset,
        "the companion's preset controls the join"
    );
    assert_eq!(
        first["document"]["value"]["attributes"][0]["value"]["value"],
        "keep"
    );
    assert_eq!(
        first["document"]["value"], second["document"]["value"],
        "one physical header"
    );
    assert_ne!(
        first["document"]["sourceUsesRefs"], second["document"]["sourceUsesRefs"],
        "distinct Child selectors"
    );
    assert_preset_owner(&uses, &first["document"]["value"]["presetGuid"], &file);
    assert_payload_reads(&placements, &file, &wrong);
}

fn assert_payload_reads(placements: &Placements, selected: &str, decoy: &str) {
    let related = |path: &str| {
        placements
            .ledger
            .entries
            .iter()
            .filter(move |entry| entry.path.as_str().replace('\\', "/").ends_with(path))
            .count()
    };
    assert_eq!(
        related(&format!("!{selected}")),
        1,
        "the companion payload is read once"
    );
    assert_eq!(
        related(&format!("!{decoy}")),
        0,
        "a matching basename outside HCD is ignored"
    );
}

fn assert_preset_owner(uses: &SourceUses, preset: &Value, file: &str) {
    let payload = preset["sourceUsesRefs"][0]
        .as_str()
        .expect("exact preset source reference");
    let inventory = uses.freeze().expect("complete source inventory");
    let origin = inventory
        .uses()
        .iter()
        .find(|item| item.id().as_str() == payload)
        .expect("source use resolves");
    let original = inventory
        .sources()
        .iter()
        .find(|item| item.id() == origin.source())
        .expect("physical source resolves");
    let source = serde_json::to_value(original).expect("original source inventory row");
    let path = source["path"]["value"]
        .as_str()
        .expect("original nested path");
    assert!(path.replace('\\', "/").ends_with(file));
}

#[test]
fn context_states_do_not_turn_missing_or_ambiguous_entries_into_payloads() {
    let dir = tempfile::tempdir().expect("a directory");
    let duplicate = text_of(30);
    let missing = text_of(31);
    let malformed = text_of(32);
    let bad_crc = text_of(33);
    let children = vec![
        location(&duplicate, "1"),
        location(&missing, "1"),
        location(&duplicate, "false"),
        location(&duplicate, "bad"),
        location("bad", "1"),
        location(&malformed, "1"),
        location(&bad_crc, "1"),
    ];
    let xml = root(children);
    let header = b"<HierarchicalContextData/>".as_slice();
    let inner = [
        TestEntry::stored(format!("a/HCD/{duplicate}.xml"), header),
        TestEntry::stored(format!("b/HCD/{duplicate}.xml"), header),
        TestEntry::stored(format!("b/HCD/{malformed}.xml"), b"<Other/>".as_slice()),
        TestEntry::stored(format!("b/HCD/{bad_crc}.xml"), header).with_crc32(0),
    ];
    let mut placements = read(
        dir.path(),
        vec![root_container(&xml, inner)],
        Limits::default(),
    );
    let rows = placements.take_locations();
    let contexts: Vec<_> = rows.rows.iter().map(context).collect();
    let [
        duplicate,
        missing,
        disabled,
        invalid_flag,
        invalid_guid,
        malformed,
        unreadable,
    ] = contexts.as_slice()
    else {
        panic!("one row per Child");
    };
    assert_eq!(duplicate["failure"]["value"], "ambiguous");
    assert_eq!(
        duplicate["candidates"]["value"].as_array().map(Vec::len),
        Some(2)
    );
    assert_eq!(duplicate["document"]["state"], "absent");
    assert_eq!(missing["failure"]["value"], "dangling");
    assert_eq!(missing["candidates"]["value"], serde_json::json!([]));
    assert!(
        disabled.is_null(),
        "the authored false flag requires no lookup"
    );
    for invalid in [invalid_flag, invalid_guid] {
        assert_eq!(invalid["document"]["state"], "invalid");
        assert_eq!(invalid["candidates"]["state"], "absent");
    }
    assert_eq!(malformed["document"]["state"], "invalid");
    assert_eq!(unreadable["document"]["state"], "unreadable");
    assert_eq!(unreadable["document"]["sourceSetComplete"], false);
}

fn location(guid: &str, flag: &str) -> TestElement {
    TestElement::new("Child")
        .attribute("locationType", "3")
        .attribute("guid", guid)
        .attribute("hasHierarchicalContextData", flag)
}

fn root_container(xml: &str, members: impl IntoIterator<Item = TestEntry>) -> TestEntry {
    let container = TestContainer::new().entry(TestEntry::stored("tsystem.xml", xml.as_bytes()));
    let container = members.into_iter().fold(container, TestContainer::entry);
    TestEntry::stored(format!("{FOLDER}tsystem.socpak"), container.build())
}

fn context(row: &LocationInstance) -> Value {
    serde_json::to_value(row).expect("location wire row")["declaration"]["hierarchicalContext"]
        .take()
}
