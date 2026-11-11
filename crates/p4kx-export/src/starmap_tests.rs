// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::{Archive, Guid};
use p4kx_testkit::{ArchiveBuilder, TestContainer, TestElement, TestEntry, test_key};

use crate::starmap_containers::{Limits, Placements};
use crate::starmap_index::Index;
use crate::starmap_parse::read_all;

const FOLDER: &str = "Data\\ObjectContainers\\PU\\system\\t\\";

fn guid(value: u128) -> Guid {
    let text = format!("{value:032x}");
    format!(
        "{}-{}-{}-{}-{}",
        &text[0..8],
        &text[8..12],
        &text[12..16],
        &text[16..20],
        &text[20..32]
    )
    .parse()
    .expect("a GUID")
}

fn text_of(record: u128) -> String {
    guid(record).to_string()
}

fn child(position: &str, record: u128) -> TestElement {
    TestElement::new("Child")
        .attribute("pos", position)
        .attribute("starMapRecord", text_of(record))
        .attribute("entityName", format!("e{record}"))
}

fn external(name: &str) -> TestElement {
    external_at(name, "0,0,0")
}

fn external_at(name: &str, position: &str) -> TestElement {
    TestElement::new("Child")
        .attribute("pos", position)
        .attribute("external", "1")
        .attribute(
            "name",
            format!("Data/objectcontainers/pu/system/t/{name}.socpak"),
        )
}

fn root(children: Vec<TestElement>) -> String {
    TestElement::new("ObjectContainer")
        .child(children.into_iter().fold(
            TestElement::new("ChildObjectContainers"),
            TestElement::child,
        ))
        .to_xml()
}

fn socpak(name: &str, xml: &str) -> TestEntry {
    TestEntry::stored(
        format!("{FOLDER}{name}.socpak"),
        TestContainer::new()
            .entry(TestEntry::stored(format!("{name}.xml"), xml.as_bytes()))
            .build(),
    )
}

fn open(dir: &Path, entries: Vec<TestEntry>) -> Archive {
    let path = dir.join("Data.p4k");
    entries
        .into_iter()
        .fold(ArchiveBuilder::new(), ArchiveBuilder::entry)
        .write_to(&path)
        .expect("a written Archive");
    Archive::open_with_key(path, Some(test_key())).expect("an opened Archive")
}

fn read(dir: &Path, entries: Vec<TestEntry>, limits: Limits) -> Placements {
    let archive = open(dir, entries);
    crate::catalog_read::original::with_entry_capture(&archive, |capture| {
        Placements::read_within(&archive, None, limits, capture)
    })
    .expect("placements")
}

fn system(xml: &str, others: &[(&str, String)]) -> Vec<TestEntry> {
    let mut entries = vec![socpak("tsystem", xml)];
    entries.extend(others.iter().map(|(name, xml)| socpak(name, xml)));
    entries
}

fn placed(placements: &Placements, record: u128) -> Option<[f64; 3]> {
    placements.get(guid(record)).map(|place| place.position)
}

#[test]
fn a_position_beyond_a_million_billion_metres_or_overflowing_is_not_placed() {
    let dir = tempfile::tempdir().expect("a directory");
    let xml = root(vec![
        child("1e16,0,0", 1),
        child("1e15,0,0", 2),
        child("1e308,0,0", 3)
            .child(TestElement::new("ChildObjectContainers").child(child("1e308,0,0", 4))),
    ]);
    let placements = read(dir.path(), system(&xml, &[]), Limits::default());
    assert_eq!(placed(&placements, 1), None, "beyond the limit");
    assert_eq!(
        placed(&placements, 2),
        Some([1.0e15, 0.0, 0.0]),
        "at the limit"
    );
    assert_eq!(placed(&placements, 3), None, "the parent is out of range");
    assert_eq!(placed(&placements, 4), None, "so is everything below it");
    assert_eq!(placements.counts.malformed, 2, "two Children are counted");
    assert!(
        placements
            .report
            .issues
            .iter()
            .any(|line| line.contains("out of range")),
        "an issue line"
    );
}

#[test]
fn nested_children_beyond_the_depth_limit_are_counted() {
    let dir = tempfile::tempdir().expect("a directory");
    let deepest = child("1,0,0", 3);
    let middle = child("1,0,0", 2).child(TestElement::new("ChildObjectContainers").child(deepest));
    let top = child("1,0,0", 1).child(TestElement::new("ChildObjectContainers").child(middle));
    let limits = Limits {
        depth: 2,
        ..Limits::default()
    };
    let placements = read(dir.path(), system(&root(vec![top]), &[]), limits);
    assert!(placed(&placements, 1).is_some(), "depth 0");
    assert!(placed(&placements, 2).is_some(), "depth 1");
    assert_eq!(placed(&placements, 3), None, "depth 2 is beyond the limit");
    assert_eq!(placements.counts.malformed, 1, "counted once");
}

#[test]
fn children_beyond_the_budget_of_a_system_are_counted() {
    let dir = tempfile::tempdir().expect("a directory");
    let xml = root(vec![
        child("1,0,0", 1),
        child("2,0,0", 2),
        child("3,0,0", 3),
    ]);
    let limits = Limits {
        children: 2,
        ..Limits::default()
    };
    let placements = read(dir.path(), system(&xml, &[]), limits);
    assert!(placed(&placements, 2).is_some(), "within the budget");
    assert_eq!(placed(&placements, 3), None, "beyond the budget");
    assert_eq!(placements.counts.malformed, 1, "counted once");
}

#[test]
fn external_containers_beyond_the_chain_limit_are_counted() {
    let dir = tempfile::tempdir().expect("a directory");
    let entries = system(
        &root(vec![external("a")]),
        &[
            ("a", root(vec![child("1,0,0", 1), external("b")])),
            ("b", root(vec![child("2,0,0", 2)])),
        ],
    );
    let limits = Limits {
        external: 1,
        ..Limits::default()
    };
    let placements = read(dir.path(), entries, limits);
    assert!(
        placed(&placements, 1).is_some(),
        "the first external container"
    );
    assert_eq!(
        placed(&placements, 2),
        None,
        "the second one is beyond the limit"
    );
    assert_eq!(placements.counts.malformed, 1, "counted once");
}

#[test]
fn a_container_that_names_itself_is_read_once_and_counted() {
    let dir = tempfile::tempdir().expect("a directory");
    let entries = system(
        &root(vec![external("a")]),
        &[("a", root(vec![child("1,0,0", 1), external("a")]))],
    );
    let placements = read(dir.path(), entries, Limits::default());
    assert_eq!(placed(&placements, 1), Some([1.0, 0.0, 0.0]), "placed once");
    assert_eq!(placements.counts.ambiguous, 0, "not twice");
    assert_eq!(placements.counts.malformed, 1, "the cycle is counted");
}

#[test]
fn a_text_container_with_a_byte_order_mark_is_read() {
    let dir = tempfile::tempdir().expect("a directory");
    let xml = format!("\u{feff}{}", root(vec![child("4,5,6", 1)]));
    let placements = read(dir.path(), system(&xml, &[]), Limits::default());
    assert_eq!(placed(&placements, 1), Some([4.0, 5.0, 6.0]), "placed");
    assert_eq!(placements.counts.unreadable, 0, "readable");
}

#[test]
fn the_report_keeps_a_bounded_number_of_short_lines() {
    let dir = tempfile::tempdir().expect("a directory");
    let long = "x".repeat(5000);
    let mut children: Vec<TestElement> = (0..400).map(|at| child("not a position", at)).collect();
    children.push(
        TestElement::new("Child")
            .attribute("pos", "1e99999")
            .attribute("entityName", long),
    );
    let placements = read(dir.path(), system(&root(children), &[]), Limits::default());
    assert_eq!(placements.counts.malformed, 401, "every Child is counted");
    assert_eq!(placements.report.issues.len(), 300, "the lines are capped");
    assert!(
        placements
            .report
            .issues
            .iter()
            .all(|line| line.chars().count() <= 300),
        "and short"
    );
}

#[test]
fn a_container_instanced_several_times_is_composed_at_each_place() {
    let dir = tempfile::tempdir().expect("a directory");
    let entries = system(
        &root(vec![
            external_at("ring", "10,0,0"),
            external_at("ring", "10,0,0"),
            external_at("moved", "10,0,0"),
            external_at("moved", "30,0,0"),
        ]),
        &[
            ("ring", root(vec![child("1,0,0", 1)])),
            ("moved", root(vec![child("2,0,0", 2)])),
        ],
    );
    let placements = read(dir.path(), entries, Limits::default());
    assert_eq!(placed(&placements, 1), Some([11.0, 0.0, 0.0]), "same place");
    assert_eq!(placed(&placements, 2), None, "two places");
    assert_eq!(placements.counts.ambiguous, 1, "only the moved one");
    assert_eq!(placements.counts.unreadable, 0, "read");
}

#[test]
fn a_container_instanced_several_times_is_parsed_once() {
    let dir = tempfile::tempdir().expect("a directory");
    let entries = system(
        &root(vec![
            external_at("ring", "10,0,0"),
            external_at("ring", "20,0,0"),
            external_at("ring", "30,0,0"),
        ]),
        &[
            ("ring", root(vec![external("leaf")])),
            ("leaf", root(vec![])),
        ],
    );
    let archive = open(dir.path(), entries);
    let index = Index::new(&archive);
    let roots: Vec<usize> = index.systems().iter().map(|(_, entry)| *entry).collect();
    let limits = Limits::default();
    let read = crate::catalog_read::original::with_entry_capture(&archive, |capture| {
        read_all(
            &archive,
            &index,
            &roots,
            None,
            limits.external,
            (limits.depth, limits.children),
            capture,
        )
    })
    .expect("original Entry capture");
    assert_eq!(read.parsed, 3, "the system, the ring and the leaf");
    assert_eq!(read.bodies.len(), 3, "each held once");
}

#[test]
fn a_container_without_the_main_xml_name_is_read_and_reported() {
    let dir = tempfile::tempdir().expect("a directory");
    let xml = root(vec![child("4,5,6", 1)]);
    let odd = TestEntry::stored(
        format!("{FOLDER}odd.socpak"),
        TestContainer::new()
            .entry(TestEntry::stored("other.xml", xml.as_bytes()))
            .build(),
    );
    let entries = vec![socpak("tsystem", &root(vec![external("odd")])), odd];
    let placements = read(dir.path(), entries, Limits::default());
    assert_eq!(placed(&placements, 1), Some([4.0, 5.0, 6.0]), "placed");
    assert_eq!(placements.counts.unreadable, 0, "readable");
    assert!(
        placements
            .report
            .issues
            .iter()
            .any(|line| line.contains("no XML named like the container")),
        "an issue line"
    );
}

#[test]
fn a_second_child_list_is_reported() {
    let dir = tempfile::tempdir().expect("a directory");
    let xml = TestElement::new("ObjectContainer")
        .child(TestElement::new("ChildObjectContainers").child(child("1,0,0", 1)))
        .child(TestElement::new("ChildObjectContainers").child(child("2,0,0", 2)))
        .to_xml();
    let placements = read(dir.path(), system(&xml, &[]), Limits::default());
    assert!(placed(&placements, 1).is_some(), "the first list is read");
    assert_eq!(placed(&placements, 2), None, "the second is not");
    assert!(
        placements
            .report
            .issues
            .iter()
            .any(|line| line.contains("more than one ChildObjectContainers")),
        "an issue line"
    );
}
