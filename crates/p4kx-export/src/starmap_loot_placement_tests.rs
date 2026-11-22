// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::{Archive, CancelToken};
use p4kx_testkit::{ArchiveBuilder, TestContainer, TestEntry};
use serde_json::Value;

use super::{PlacementScope, Placements};
use crate::catalog_read::original::with_entry_capture;
use crate::loot_walk;
use crate::starmap_index::Index;
use crate::{CatalogError, StarmapTraversalBudget};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const ROOT: &str = "Data/ObjectContainers/PU/system/demo/demosystem.socpak";
const OCCURRENCE: &str = "Data/ObjectContainers/PU/synthetic/occurrence.socpak";
const FALLBACK: &str = "Data/ObjectContainers/PU/system/demo/fallback.socpak";
const NESTED: &str = "Data/ObjectContainers/PU/synthetic/nested.socpak";
const UNREACHED: &str = "Data/ObjectContainers/PU/synthetic/unreached.socpak";
const MEMBER: &str = "entdata/repeated.entxml";

#[test]
fn descriptor_only_loot_dependencies_preserve_canonical_documents_and_physical_order() -> TestResult
{
    let directory = tempfile::tempdir()?;
    let archive = fixture(directory.path())?;
    let (expected, document_rows) = read(&archive, false)?;
    let (actual, selected_rows) = read(&archive, true)?;

    assert!(
        actual
            .opened
            .iter()
            .map(crate::starmap_walk::Opened::key)
            .eq(expected.opened.iter().map(crate::starmap_walk::Opened::key)),
        "descriptor and fallback seed order"
    );
    assert_eq!(
        actual.counts, expected.counts,
        "descriptor route coverage remains visible"
    );
    assert!(
        !expected.instances.is_empty(),
        "the placement producer builds Container rows"
    );
    assert!(
        actual.instances.is_empty(),
        "the Loot seed walk builds no Container rows"
    );
    assert_eq!(
        selected_rows, document_rows,
        "canonical document values and their source uses"
    );
    let rows = selected_rows
        .pointer("/rows")
        .and_then(Value::as_array)
        .ok_or("original document rows")?;
    assert_eq!(
        rows.len(),
        4,
        "external, fallback aliases and nested documents appear once"
    );
    let [_, first_alias, second_alias, _] = rows.as_slice() else {
        return Err("ordered physical document aliases".into());
    };
    assert_ne!(
        first_alias.pointer("/document/value/entryIndex"),
        second_alias.pointer("/document/value/entryIndex"),
        "equal nested filenames retain distinct physical indices"
    );
    Ok(())
}

#[test]
fn descriptor_only_loot_dependencies_preserve_scope_and_cancellation() -> TestResult {
    let directory = tempfile::tempdir()?;
    let archive = fixture(directory.path())?;
    with_entry_capture(&archive, |capture| {
        let budget = StarmapTraversalBudget::new().with_placed_children_per_system(0);
        let limited = Placements::read_loot_seeds(
            &archive,
            None,
            budget,
            capture,
            PlacementScope::RecordSelection,
        )?;
        assert!(
            limited.placement_routes_incomplete,
            "an exhausted caller allowance cannot prove reachability"
        );
        let inventory = Placements::read_loot_seeds(
            &archive,
            None,
            StarmapTraversalBudget::new(),
            capture,
            PlacementScope::ArchiveInventory,
        )?;
        assert!(
            inventory.opened.is_empty(),
            "archive-inventory mode does not admit Loot seeds"
        );
        let cancelled = CancelToken::new();
        cancelled.cancel();
        assert!(
            matches!(
                Placements::read_loot_seeds(
                    &archive,
                    Some(&cancelled),
                    StarmapTraversalBudget::new(),
                    capture,
                    PlacementScope::RecordSelection
                ),
                Err(CatalogError::Cancelled)
            ),
            "a cancelled dependency read cannot certify an empty result"
        );
        Ok(())
    })?;
    Ok(())
}

fn read(archive: &Archive, descriptors_only: bool) -> TestResult<(Placements, Value)> {
    with_entry_capture(archive, |capture| {
        let scope = PlacementScope::RecordSelection;
        let placements = if descriptors_only {
            Placements::read_loot_seeds(
                archive,
                None,
                StarmapTraversalBudget::new(),
                capture,
                scope,
            )?
        } else {
            Placements::read(archive, None, capture, scope)?
        };
        let index = Index::new(archive);
        let explored = loot_walk::explore(archive, &index, &placements.opened, None, capture)?;
        let rows = serde_json::to_value(explored.original_documents).map_err(CatalogError::Json)?;
        Ok((placements, rows))
    })
    .map_err(Into::into)
}

fn fixture(directory: &Path) -> TestResult<Archive> {
    let path = directory.join("Data.p4k");
    let root = format!(
        r#"<ObjectContainer><ChildObjectContainers>
        <Child name="{OCCURRENCE}" external="1" pos="1,0,0"/>
        <Child name="{OCCURRENCE}" external="1" pos="2,0,0"/>
        </ChildObjectContainers></ObjectContainer>"#
    );
    ArchiveBuilder::new()
        .entry(container(ROOT, "demosystem.xml", &root, []))
        .entry(container(
            OCCURRENCE,
            "occurrence.xml",
            "<ObjectContainer/>",
            [entity(NESTED)],
        ))
        .entry(container(
            FALLBACK,
            "fallback.xml",
            "<ObjectContainer/>",
            [entity(NESTED), entity(NESTED)],
        ))
        .entry(container(
            NESTED,
            "nested.xml",
            "<ObjectContainer/>",
            [entity(NESTED)],
        ))
        .entry(container(
            UNREACHED,
            "unreached.xml",
            "<ObjectContainer/>",
            [entity(NESTED)],
        ))
        .write_to(&path)?;
    Ok(Archive::open(path)?)
}

fn entity(path: &str) -> TestEntry {
    TestEntry::stored(MEMBER, format!(r#"<Entity objectContainer="{path}"/>"#))
}

fn container(
    path: &str,
    descriptor: &str,
    xml: &str,
    members: impl IntoIterator<Item = TestEntry>,
) -> TestEntry {
    let archive = TestContainer::new().entry(TestEntry::stored(descriptor, xml));
    let archive = members.into_iter().fold(archive, TestContainer::entry);
    TestEntry::stored(path, archive.build())
}
