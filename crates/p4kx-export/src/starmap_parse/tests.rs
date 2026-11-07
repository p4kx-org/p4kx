// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Archive;
use p4kx_testkit::{ArchiveBuilder, TestContainer, TestEntry};

use crate::Kind;
use crate::export_entry_ledger::EntryOperation;
use crate::export_ledger::SourceDisposition;
use crate::starmap_index::Index;

use super::Body;

type TestResult = Result<(), Box<dyn std::error::Error>>;
const ROOT: &str = r"Data\ObjectContainers\PU\system\t\tsystem.socpak";
const NEXT: &str = r"Data\ObjectContainers\PU\system\t\next.socpak";

fn container(path: &str, xml: &str) -> TestEntry {
    TestEntry::stored(
        path,
        TestContainer::new()
            .entry(TestEntry::stored("main.xml", xml))
            .build(),
    )
}

fn opened(
    entries: Vec<TestEntry>,
) -> Result<(tempfile::TempDir, Archive), Box<dyn std::error::Error>> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    entries
        .into_iter()
        .fold(ArchiveBuilder::new(), ArchiveBuilder::entry)
        .write_to(&path)?;
    Ok((folder, Archive::open(path)?))
}

#[test]
fn a_valid_empty_source_does_not_hit_a_nonexistent_child_depth_limit() -> TestResult {
    for text in [
        "<ObjectContainer/>",
        "<ObjectContainer><ChildObjectContainers/></ObjectContainer>",
    ] {
        let (_folder, archive) = opened(vec![container(ROOT, text)])?;
        let source = super::read_all(&archive, &Index::new(&archive), &[0], None, 0, (0, 0));
        assert!(
            source.ledger.incomplete_inventories.is_empty(),
            "absent or empty child lists do not hide sources"
        );
        let [outer, xml] = source.ledger.entries.as_slice() else {
            return Err("actual container and XML operations expected".into());
        };
        assert_eq!(
            outer.disposition,
            SourceDisposition::Projected,
            "container directory opened"
        );
        assert_eq!(
            xml.disposition,
            SourceDisposition::Projected,
            "no child list means no truncated children"
        );
        assert_eq!(
            xml.operation,
            EntryOperation::ContainerXml,
            "actual selected XML projection"
        );
        assert_eq!(
            xml.container_entry_index,
            Some(0),
            "actual nested Entry index"
        );
    }
    Ok(())
}

#[test]
fn readable_bytes_with_invalid_xml_are_a_projection_failure() -> TestResult {
    let (_folder, archive) = opened(vec![container(
        ROOT,
        "<ObjectContainer><broken></ObjectContainer>",
    )])?;
    let source = super::read_all(&archive, &Index::new(&archive), &[0], None, 0, (64, 10));
    assert!(
        matches!(source.bodies.get(&0), Some(Body::Unparsed)),
        "the parser rejected the source"
    );
    let [outer, xml] = source.ledger.entries.as_slice() else {
        return Err("outer and inner observations expected".into());
    };
    assert_eq!(
        outer.disposition,
        SourceDisposition::Projected,
        "opening the container succeeded"
    );
    assert_eq!(
        xml.disposition,
        SourceDisposition::ProjectionFailed,
        "bytes were read but the typed XML projection failed"
    );
    Ok(())
}

#[test]
fn a_truncated_projection_keeps_its_readable_children_and_its_failure() -> TestResult {
    let xml = r#"<ObjectContainer><ChildObjectContainers><Child pos="0,0,0"/><Child pos="1,0,0"/></ChildObjectContainers></ObjectContainer>"#;
    let (_folder, archive) = opened(vec![container(ROOT, xml)])?;
    let source = super::read_all(&archive, &Index::new(&archive), &[0], None, 0, (64, 1));
    let Some(Body::Parsed { root, .. }) = source.bodies.get(&0) else {
        return Err("readable partial tree expected".into());
    };
    assert_eq!(
        root.children.len(),
        1,
        "readable selected child remains present"
    );
    assert!(root.truncated, "the actual budget loss is retained");
    assert!(
        source
            .ledger
            .incomplete_inventories
            .contains(&Kind::StarmapObjects),
        "omitted children can hide additional source containers"
    );
    assert!(
        source
            .ledger
            .entries
            .iter()
            .any(|entry| entry.operation == EntryOperation::ContainerXml
                && entry.disposition == SourceDisposition::ProjectionFailed),
        "retaining data cannot certify completeness"
    );
    Ok(())
}

#[test]
fn a_known_container_beyond_the_walk_limit_is_not_claimed_as_read() -> TestResult {
    let xml = format!(
        r#"<ObjectContainer><ChildObjectContainers><Child pos="0,0,0" external="1" name="{NEXT}"/></ChildObjectContainers></ObjectContainer>"#
    );
    let (_folder, archive) = opened(vec![
        container(ROOT, &xml),
        container(NEXT, "<ObjectContainer/>"),
    ])?;
    let source = super::read_all(&archive, &Index::new(&archive), &[0], None, 0, (64, 10));
    assert!(
        !source.bodies.contains_key(&1),
        "the bounded walk did not parse the next container"
    );
    assert!(
        source
            .ledger
            .incomplete_inventories
            .contains(&Kind::StarmapObjects),
        "unwalked sources make the inventory incomplete"
    );
    let unread = source
        .ledger
        .entries
        .iter()
        .find(|entry| entry.entry_index == Some(1))
        .ok_or("unread known source identity")?;
    assert_eq!(
        unread.operation,
        EntryOperation::Enumeration,
        "limit loss is separate from a directory operation"
    );
    assert_eq!(
        unread.disposition,
        SourceDisposition::ReadFailed,
        "the required source was not read"
    );
    assert_eq!(
        unread.container_entry_index, None,
        "no invented inner source"
    );
    Ok(())
}

#[test]
fn a_missing_external_container_preserves_the_authored_path() -> TestResult {
    let xml = format!(
        r#"<ObjectContainer><ChildObjectContainers><Child pos="0,0,0" external="1" name="{NEXT}"/></ChildObjectContainers></ObjectContainer>"#
    );
    let (_folder, archive) = opened(vec![container(ROOT, &xml)])?;
    let source = super::read_all(&archive, &Index::new(&archive), &[0], None, 3, (64, 10));
    let missing = source
        .ledger
        .entries
        .iter()
        .find(|entry| entry.entry_index.is_none())
        .ok_or("missing configured source identity")?;
    assert_eq!(missing.path, NEXT, "the declared path survives");
    assert_eq!(
        missing.disposition,
        SourceDisposition::ReadFailed,
        "absence cannot look like successful empty data"
    );
    Ok(())
}
