// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_extraction::{Archive, SourceValueState};
use p4kx_testkit::{ArchiveBuilder, TestContainer, TestEntry};

use super::SystemOrigins;
use crate::CatalogError;
use crate::catalog_read::original::with_entry_capture;
use crate::export_entry_ledger::EntryOperation;
use crate::export_ledger::SourceDisposition;
use crate::loot_entxml::documents::LootEntityContainerDocument;
use crate::loot_walk::{self, DocumentVisit};
use crate::starmap_containers::{Limits, PlacementScope, Placements};
use crate::starmap_index::{self, Index};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;
type DocumentCallback = fn(usize, LootEntityContainerDocument) -> Result<(), CatalogError>;

const HOME: &str = "Data/ObjectContainers/PU/system/home/homesystem.socpak";
const FOREIGN: &str = "Data/ObjectContainers/PU/system/foreign/foreignsystem.socpak";
const ANCHOR: &str = "Data/ObjectContainers/PU/System/Home/ownership.socpak";
const EARLY: &str = "Data/ObjectContainers/PU/system/home/a.socpak";
const LATER: &str = "Data/ObjectContainers/PU/system/home/b.socpak";
const NESTED: &str = "Data/ObjectContainers/PU/synthetic/nested.socpak";
const NON_CONTAINER: &str = "Data/ObjectContainers/PU/system/home/notes.xml";

#[test]
fn system_anchor_is_classified_from_its_exact_directory_slot() -> TestResult {
    let (_folder, archive) = fixture()?;
    let anchor = slot(&archive, ANCHOR)?;
    let invalid = slot(&archive, NON_CONTAINER)?;
    with_entry_capture(&archive, |capture| {
        let mut origins = SystemOrigins::try_new(&archive, capture, &Index::new(&archive))?;
        let system = origins.at(anchor)?;
        assert_eq!(
            system.value().map(String::as_str),
            Some("home"),
            "the requested physical filename determines the normalized folder"
        );
        assert!(
            system.origin().source_set_complete(),
            "the original slot and complete root survey establish the folder"
        );
        let other = origins.at(invalid)?;
        assert_eq!(
            other.state(),
            SourceValueState::Invalid,
            "an exact non-container filename cannot name a system anchor"
        );
        let missing = origins.at(archive.entries().len())?;
        assert_eq!(
            missing.state(),
            SourceValueState::Absent,
            "the original directory count excludes an out-of-range slot"
        );
        assert!(
            missing.origin().source_set_complete()
                && !missing.origin().source_uses_refs().is_empty(),
            "absence carries original root directory evidence"
        );
        Ok(())
    })?;
    Ok(())
}

#[test]
fn exact_system_walk_exhausts_selected_links_after_global_fallback_suppression() -> TestResult {
    let (_folder, archive) = fixture()?;
    let anchor = slot(&archive, ANCHOR)?;
    let early = slot(&archive, EARLY)?;
    let later = slot(&archive, LATER)?;
    let nested = slot(&archive, NESTED)?;
    let home = slot(&archive, HOME)?;
    let index = Index::new(&archive);
    with_entry_capture(&archive, |capture| {
        let mut systems = SystemOrigins::try_new(&archive, capture, &index)?;
        let folder = systems.at(anchor)?.value().map(String::as_str);
        let placements = Placements::read_loot_seeds(
            &archive,
            None,
            Limits::default(),
            capture,
            PlacementScope::RecordSelection,
        )?;
        assert!(
            placements
                .opened
                .iter()
                .any(|root| root.entry == anchor && root.system.as_ref() == "foreign"),
            "the home-folder anchor belongs exclusively to the foreign actual open"
        );
        let walked = loot_walk::visit_documents(
            &archive,
            &index,
            placements
                .opened
                .iter()
                .filter(|root| Some(root.system.as_ref()) == folder)
                .map(|root| root.entry),
            None,
            capture,
            None::<DocumentVisit<DocumentCallback>>,
        )?;
        assert_eq!(
            walked.summaries.len(),
            4,
            "only the selected root, fallback seeds and linked descendants are read"
        );
        for entry in [home, early, later, nested] {
            assert!(
                walked.summaries.contains_key(&entry),
                "each selected physical source was reached"
            );
        }
        assert!(
            walked
                .summaries
                .get(&later)
                .is_some_and(|source| source.nested == [nested]),
            "links remaining after an earlier EntXML failure are still traversed"
        );
        assert_unreadable_earlier_route(&walked, early);
        Ok(())
    })?;
    Ok(())
}

fn assert_unreadable_earlier_route(walked: &loot_walk::Walked, early: usize) {
    assert!(
        !walked.routes_complete && !walked.documents_complete,
        "an unreadable earlier candidate blocks a complete winner or miss"
    );
    assert!(
        walked.ledger.entries.iter().any(|observed| {
            observed.entry_index == Some(early)
                && observed.container_entry_index.is_some()
                && observed.operation == EntryOperation::EntityLinks
                && observed.disposition == SourceDisposition::ProjectionFailed
        }),
        "the unreadable original source operation survives link-only traversal"
    );
}

#[test]
fn a_nested_directory_cannot_prove_a_missing_root_system_anchor() -> TestResult {
    let (_folder, archive) = fixture()?;
    let outer = archive
        .entries()
        .get(slot(&archive, ANCHOR)?)
        .ok_or("original SOC archive")?;
    let nested = archive.open_container(outer)?;
    let index = Index::new(&nested);
    with_entry_capture(&nested, |capture| {
        let mut origins = SystemOrigins::try_new(&nested, capture, &index)?;
        let missing = origins.at(nested.entries().len())?;
        assert_eq!(
            missing.state(),
            SourceValueState::Unreadable,
            "a nested directory has no complete root directory receipt"
        );
        assert!(
            !missing.origin().source_set_complete(),
            "the scope cannot certify an absent root entry"
        );
        Ok(())
    })?;
    Ok(())
}

fn fixture() -> TestResult<(tempfile::TempDir, Archive)> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    let foreign = format!(
        r#"<ObjectContainer><ChildObjectContainers>
        <Child external="1" name="{ANCHOR}" pos="1,0,0"/>
        </ChildObjectContainers></ObjectContainer>"#
    );
    let linked = format!(r#"<Entity objectContainer="{NESTED}"/>"#);
    ArchiveBuilder::new()
        .entry(container(
            HOME,
            "homesystem.xml",
            "<ObjectContainer/>",
            None,
        ))
        .entry(container(FOREIGN, "foreignsystem.xml", &foreign, None))
        .entry(container(
            ANCHOR,
            "ownership.xml",
            "<ObjectContainer/>",
            Some("not selected"),
        ))
        .entry(container(
            EARLY,
            "a.xml",
            "<ObjectContainer/>",
            Some("not XML"),
        ))
        .entry(container(
            LATER,
            "b.xml",
            "<ObjectContainer/>",
            Some(&linked),
        ))
        .entry(container(NESTED, "nested.xml", "<ObjectContainer/>", None))
        .entry(TestEntry::stored(NON_CONTAINER, []))
        .write_to(&path)?;
    Ok((folder, Archive::open(path)?))
}

fn container(path: &str, file: &str, descriptor: &str, entity: Option<&str>) -> TestEntry {
    let mut content = TestContainer::new().entry(TestEntry::stored(file, descriptor));
    if let Some(entity) = entity {
        content = content.entry(TestEntry::stored("entdata/source.entxml", entity));
    }
    TestEntry::stored(path, content.build())
}

fn slot(archive: &Archive, path: &str) -> TestResult<usize> {
    let wanted = starmap_index::key(path);
    archive
        .entries()
        .iter()
        .find(|entry| starmap_index::key(entry.path()) == wanted)
        .map(p4kx_extraction::Entry::index)
        .ok_or_else(|| "exact fixture entry is missing".into())
}
