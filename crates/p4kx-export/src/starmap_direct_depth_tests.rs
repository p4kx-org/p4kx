// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_testkit::{CryXmlByteOrder, CryXmlSignature, TestContainer, TestElement, TestEntry};

use super::{FOLDER, Limits, child, external_at, guid, read, text_of};
use crate::export_entry_ledger::EntryOperation;
use crate::export_ledger::SourceDisposition;
use crate::starmap_containers::proof::PlacementStep;
use crate::starmap_report::Placement;

#[test]
fn binary_xml_places_and_follows_external_sources_beyond_64_inline_levels()
-> Result<(), Box<dyn Error>> {
    const DEPTH: u16 = 70;
    let folder = tempfile::tempdir()?;
    let system = binary_container("tsystem", inline_chain(DEPTH));
    let leaf = binary_container("leaf", child("2,0,0", 2));
    let placements = read(folder.path(), vec![system, leaf], Limits::default());

    let terminal = placements.get(guid(1)).ok_or("the deep inline placement")?;
    let external = placements.get(guid(2)).ok_or("the external placement")?;
    assert_eq!(
        terminal.position,
        [f64::from(DEPTH), 0.0, 0.0],
        "inline transforms"
    );
    assert_eq!(
        external.position,
        [f64::from(DEPTH + 2), 0.0, 0.0],
        "external transform"
    );
    assert_routes(terminal, external, DEPTH);
    assert_eq!(
        placements
            .ledger
            .entries
            .iter()
            .filter(|entry| entry.operation == EntryOperation::ContainerXml
                && entry.disposition == SourceDisposition::Projected)
            .count(),
        2,
        "both physical main XML sources are read and projected",
    );
    assert_eq!(
        placements.counts.malformed, 0,
        "the complete routes are placed"
    );
    assert!(
        placements.ledger.incomplete_inventories.is_empty(),
        "the inventory retains all reachable sources"
    );
    Ok(())
}

fn inline_chain(depth: u16) -> TestElement {
    let terminal = external_at("leaf", "1,0,0").attribute("starMapRecord", text_of(1));
    (1..depth).fold(terminal, |below, _| {
        TestElement::new("Child")
            .attribute("pos", "1,0,0")
            .child(TestElement::new("ChildObjectContainers").child(below))
    })
}

fn binary_container(name: &str, child: TestElement) -> TestEntry {
    let binary = TestElement::new("ObjectContainer")
        .child(TestElement::new("ChildObjectContainers").child(child))
        .to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little);
    TestEntry::stored(
        format!("{FOLDER}{name}.socpak"),
        TestContainer::new()
            .entry(TestEntry::stored(format!("{name}.xml"), binary))
            .build(),
    )
}

fn assert_routes(terminal: &Placement, external: &Placement, depth: u16) {
    let original_nodes: Vec<_> = (1..=u32::from(depth)).rev().map(|n| n * 2).collect();
    assert_eq!(
        terminal
            .proof
            .last
            .steps()
            .map(PlacementStep::child_index)
            .collect::<Vec<_>>(),
        original_nodes,
        "all original main XML Child coordinates are kept in leaf-to-root order",
    );
    let mut steps = external.proof.last.steps().map(PlacementStep::child_index);
    assert_eq!(
        steps.next(),
        Some(2),
        "the leaf refers to the original Child in the other XML"
    );
    assert_eq!(
        steps.collect::<Vec<_>>(),
        original_nodes,
        "the external occurrence retains the original inline route"
    );
    assert!(
        external.proof.last.via.is_some(),
        "the final step holds the original external path selector"
    );
}
