// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_extraction::Archive;
use p4kx_testkit::{ArchiveBuilder, TestContainer, TestEntry};

use super::super::selection::first_placements;
use super::{CONFIG, opened};
use crate::CatalogError;
use crate::catalog_read::original::with_entry_capture;
use crate::export_entry_ledger::EntryOperation;
use crate::harvest_locations::Objects;
use crate::loot_container_source::EntXmlAuthor;
use crate::loot_entxml::documents::LootEntityContainerDocument;
use crate::loot_walk::{self, DocumentVisit, Placed, PlacedTarget};
use crate::starmap_index::Index;

const OWNER: &str = "Data/ObjectContainers/PU/test/owner.socpak";
const DUPLICATE: &str = "entdata/duplicate.entxml";

type TestResult<T = ()> = Result<T, Box<dyn Error>>;
type DocumentCallback = fn(usize, LootEntityContainerDocument) -> Result<(), CatalogError>;

#[test]
fn duplicate_config_only_entxml_members_keep_the_first_physical_author() -> TestResult {
    let (_folder, archive) = fixture()?;
    let index = Index::new(&archive);
    let owner = index.find(OWNER).ok_or("original containing archive")?;
    let root = opened(owner, "test");
    let walked = with_entry_capture(&archive, |capture| {
        loot_walk::visit_documents(
            &archive,
            &index,
            std::iter::once(owner),
            None,
            capture,
            None::<DocumentVisit<DocumentCallback>>,
        )
    })?;
    let summary = walked.summaries.get(&owner).ok_or("owner summary")?;
    let [earlier, later] = summary.placed.as_slice() else {
        return Err("each duplicate filename must retain its distinct placement".into());
    };
    assert_author(earlier, owner, 0);
    assert_author(later, owner, 2);
    assert!(
        earlier.path.is_empty() && later.path.is_empty(),
        "config-only documents retain the empty canonical container path"
    );
    let entries: Vec<_> = walked
        .ledger
        .entries
        .iter()
        .filter(|operation| operation.operation == EntryOperation::EntityLinks)
        .map(|operation| (operation.entry_index, operation.container_entry_index))
        .collect();
    assert_eq!(
        entries,
        [(Some(owner), Some(0)), (Some(owner), Some(2))],
        "selected placements preserve the exact verified physical receipts"
    );
    let roots = std::slice::from_ref(&root);
    let rows: Vec<_> = first_placements(roots, &Objects::new(&[]), &walked.summaries).collect();
    let [winner] = rows.as_slice() else {
        return Err("one logical key from two physical authors".into());
    };
    assert!(
        std::ptr::eq(winner.placed, earlier),
        "the first physical author supplies the canonical row"
    );
    Ok(())
}

fn assert_author(placed: &Placed, owner: usize, member: usize) {
    assert_eq!(
        placed.author,
        EntXmlAuthor {
            containing_entry_index: owner,
            document_entry_index: member,
        },
        "physical author from the exact verified input"
    );
    assert_eq!(
        placed.target,
        PlacedTarget::Missing,
        "the authored config does not imply a selected target"
    );
}

fn fixture() -> TestResult<(tempfile::TempDir, Archive)> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    let xml = format!(r#"<Entity multiConfigRef="{CONFIG}"/>"#);
    let content = TestContainer::new()
        .entry(TestEntry::stored(DUPLICATE, xml.clone()))
        .entry(TestEntry::stored("entdata/notes.txt", "fixture"))
        .entry(TestEntry::stored(DUPLICATE, xml))
        .build();
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/notes.txt", "fixture"))
        .entry(TestEntry::stored(OWNER, content))
        .write_to(&path)?;
    Ok((folder, Archive::open(path)?))
}
