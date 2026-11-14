// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{TestContainer, TestEntry};

use super::{FOLDER, Limits, child, external, placed, read, root, socpak, text_of};
use crate::export_entry_ledger::EntryOperation;
use crate::export_ledger::SourceDisposition;

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
    let hcd = format!("hCd\\{tile}.xml");
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
