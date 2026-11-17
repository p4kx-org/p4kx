// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;
use std::sync::Arc;

use p4kx_testkit::{ArchiveBuilder, TestContainer, TestElement, TestEntry, test_key};

use super::*;
use crate::export_entry_ledger::EntryOperation;

fn explore(
    archive: &Archive,
    index: &Index<'_>,
    opened: &[Opened],
    cancel: Option<&CancelToken>,
) -> Explored {
    crate::catalog_read::original::with_entry_capture(archive, |capture| {
        super::explore(archive, index, opened, cancel, capture)
    })
    .expect("original Entry capture")
}

const CONFIG: &str = "687af3db-4c0d-4e8d-9261-102274f8b6ed";

fn guid() -> Guid {
    CONFIG.parse().expect("a GUID")
}

fn placed(path: &str, entry: Option<usize>) -> Placed {
    Placed {
        config: guid(),
        path: path.to_owned(),
        target: entry.map_or(PlacedTarget::Missing, PlacedTarget::Resolved),
    }
}

fn summary(nested: &[usize], placed_paths: &[&str]) -> Summary {
    Summary {
        nested: nested.to_vec(),
        placed: placed_paths.iter().map(|path| placed(path, None)).collect(),
        ..Summary::default()
    }
}

fn paths(found: &[&Placed]) -> Vec<String> {
    let mut paths: Vec<String> = found.iter().map(|placed| placed.path.clone()).collect();
    paths.sort_unstable();
    paths
}

#[test]
fn below_collects_the_root_and_everything_it_places() {
    let summaries = HashMap::from([
        (0, summary(&[1, 2], &["root"])),
        (1, summary(&[3], &["one"])),
        (2, summary(&[3], &["two"])),
        (3, summary(&[], &["three"])),
        (9, summary(&[], &["elsewhere"])),
    ]);
    assert_eq!(
        paths(&below(0, &summaries)),
        ["one", "root", "three", "two"],
        "a container reached twice counts once"
    );
}

#[test]
fn below_ends_a_cycle_where_it_returns() {
    let summaries = HashMap::from([(0, summary(&[1], &["a"])), (1, summary(&[0], &["b"]))]);
    assert_eq!(paths(&below(0, &summaries)), ["a", "b"]);
    assert_eq!(paths(&below(1, &summaries)), ["a", "b"]);
}

#[test]
fn below_skips_a_container_that_was_not_read() {
    let summaries = HashMap::from([(0, summary(&[7], &["a"]))]);
    assert_eq!(paths(&below(0, &summaries)), ["a"]);
    assert_eq!(below(5, &summaries).len(), 0);
}

fn entity(container: &str, config: Option<&str>) -> String {
    let mut component =
        TestElement::new("EntityComponentObjectContainer").attribute("objectContainer", container);
    if let Some(config) = config {
        component = component.attribute("multiConfigRef", config);
    }
    TestElement::new("Entity").child(component).to_xml()
}

fn socpak(name: &str, files: &[String]) -> TestEntry {
    let container = files
        .iter()
        .enumerate()
        .fold(TestContainer::new(), |container, (at, text)| {
            container.entry(TestEntry::stored(
                format!("entdata/{at}.entxml"),
                text.clone(),
            ))
        });
    TestEntry::stored(
        format!("Data\\ObjectContainers\\PU\\loc\\{name}.socpak"),
        container.build(),
    )
}

fn written(name: &str) -> String {
    format!("ObjectContainers/PU/loc/{name}.socpak")
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

fn root_of(archive: &Archive, name: &str) -> Opened {
    let index = Index::new(archive);
    Opened {
        system: Arc::from("t"),
        entry: index.find(&written(name)).expect("a container"),
        under: None,
    }
}

#[test]
fn a_cycle_of_containers_is_read_once_each() {
    let dir = tempfile::tempdir().expect("a directory");
    let archive = open(
        dir.path(),
        vec![
            socpak("a", &[entity(&written("b"), Some(CONFIG))]),
            socpak("b", &[entity(&written("a"), Some(CONFIG))]),
        ],
    );
    let index = Index::new(&archive);
    let root = root_of(&archive, "a");
    let explored = explore(&archive, &index, std::slice::from_ref(&root), None);
    assert_eq!(explored.summaries.len(), 2);
    let a = explored.summaries.get(&root.entry).expect("a");
    assert_eq!(a.placed.len(), 1);
    assert_eq!(a.placed[0].target.entry(), index.find(&written("b")));
    assert_eq!(below(root.entry, &explored.summaries).len(), 2);
}

#[test]
fn duplicate_selected_targets_are_reported_without_traversing_either() {
    let directory = tempfile::tempdir().expect("a directory");
    let archive = open(
        directory.path(),
        vec![
            socpak("a", &[entity(&written("b"), Some(CONFIG))]),
            socpak("b", &[entity(&written("first"), None)]),
            socpak("b", &[entity(&written("second"), None)]),
        ],
    );
    let index = Index::new(&archive);
    let root = root_of(&archive, "a");
    let explored = explore(&archive, &index, std::slice::from_ref(&root), None);
    assert_eq!(
        explored.summaries.len(),
        1,
        "only the unique root is visited"
    );
    let compiled = crate::export_health_entries::compile(explored.ledger.entries)
        .expect("consistent source accounting");
    let candidates: Vec<_> = compiled
        .failed
        .iter()
        .filter(|entry| {
            entry.source.kind == Kind::LootContainers
                && entry.code == crate::ExportDiagnosticCode::AmbiguousIdentity
                && entry.stage == crate::ExportDiagnosticStage::Selection
        })
        .map(|entry| entry.source.entry_index)
        .collect();
    assert_eq!(
        candidates,
        [Some(1), Some(2)],
        "both candidates retain physical identity"
    );
}

#[test]
fn a_placed_container_the_archive_lacks_has_no_entry() {
    let dir = tempfile::tempdir().expect("a directory");
    let archive = open(
        dir.path(),
        vec![socpak("a", &[entity(&written("ghost"), Some(CONFIG))])],
    );
    let index = Index::new(&archive);
    let root = root_of(&archive, "a");
    let explored = explore(&archive, &index, std::slice::from_ref(&root), None);
    let a = explored.summaries.get(&root.entry).expect("a");
    assert_eq!(a.placed, [placed(&written("ghost"), None)]);
    assert_eq!(a.nested.len(), 0);
    let missing = explored
        .ledger
        .entries
        .iter()
        .find(|entry| entry.operation == EntryOperation::Enumeration)
        .expect("authored missing Entry observation");
    assert_eq!(
        missing.kind,
        Kind::LootContainers,
        "loot owns the missing link"
    );
    assert_eq!(
        missing.disposition,
        SourceDisposition::ReadFailed,
        "a genuinely absent target is still a failed read"
    );
    assert!(missing.original().is_ok(), "actual authored link receipt");
}

#[test]
fn an_entity_file_with_several_links_is_noted() {
    let dir = tempfile::tempdir().expect("a directory");
    let two = TestElement::new("Entity")
        .child(TestElement::new("A").attribute("objectContainer", written("b")))
        .child(TestElement::new("B").attribute("objectContainer", written("c")))
        .to_xml();
    let archive = open(dir.path(), vec![socpak("a", &[two])]);
    let index = Index::new(&archive);
    let root = root_of(&archive, "a");
    let explored = explore(&archive, &index, std::slice::from_ref(&root), None);
    let a = explored.summaries.get(&root.entry).expect("a");
    assert_eq!(a.several, ["entdata\\0.entxml"]);
}

#[test]
fn all_reachable_physical_containers_are_opened_once() {
    let dir = tempfile::tempdir().expect("a directory");
    let archive = open(
        dir.path(),
        vec![
            socpak(
                "a",
                &[entity(&written("b"), None), entity(&written("c"), None)],
            ),
            socpak("b", &[entity(&written("c"), None)]),
            socpak("c", &[entity(&written("d"), Some(CONFIG))]),
            socpak("d", &[]),
            socpak("elsewhere", &[]),
        ],
    );
    let index = Index::new(&archive);
    let root = root_of(&archive, "a");
    let explored = explore(&archive, &index, std::slice::from_ref(&root), None);
    assert_eq!(explored.summaries.len(), 4, "only the reachable subgraph");
    let mut opened: Vec<_> = explored
        .ledger
        .entries
        .iter()
        .filter(|entry| entry.operation == EntryOperation::ContainerDirectory)
        .map(|entry| {
            assert_eq!(
                entry.disposition,
                SourceDisposition::Projected,
                "all reachable directories were opened"
            );
            assert!(entry.original().is_ok(), "physical directory receipt");
            entry.entry_index
        })
        .collect();
    opened.sort_unstable();
    let mut expected = ["a", "b", "c", "d"].map(|name| index.find(&written(name)));
    expected.sort_unstable();
    assert_eq!(opened, expected, "both paths to c share one physical read");
    let placed = below(root.entry, &explored.summaries);
    assert_eq!(
        paths(&placed),
        [path_of(&archive, index.find(&written("d")).expect("d"))],
        "the deepest placement is retained"
    );
}

#[test]
fn a_cancelled_walk_reads_nothing() {
    let dir = tempfile::tempdir().expect("a directory");
    let archive = open(dir.path(), vec![socpak("a", &[])]);
    let index = Index::new(&archive);
    let root = root_of(&archive, "a");
    let cancel = CancelToken::new();
    cancel.cancel();
    crate::catalog_read::original::with_entry_capture(&archive, |capture| {
        let result = super::explore(&archive, &index, &[root], Some(&cancel), capture);
        assert!(
            matches!(result, Err(CatalogError::Cancelled)),
            "a cancelled source walk must not become a complete empty result"
        );
        let inventory = capture.source_uses().freeze()?;
        assert!(
            inventory
                .iter_uses()
                .all(|used| used.scope() != p4kx_extraction::InputScope::Payload),
            "a pre-cancelled walk does not read or attempt any container payload"
        );
        Ok(())
    })
    .expect("cancelled walk source evidence");
}
