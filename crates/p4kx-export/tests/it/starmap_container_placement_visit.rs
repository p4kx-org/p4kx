// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::sdk_full_proof as full_proof;
use p4kx_export::catalog_queries::starmap::{
    ContainerPlacementEvent, ContainerPlacementRootVisit, ContainerRootStatus, DatasetAvailability,
    visit_container_placements, visit_container_placements_with_provenance_mode,
};
use p4kx_export::{CatalogError, build_catalog};
use p4kx_extraction::CancelToken;
use p4kx_testkit::{TestElement, TestEntry};
use serde_json::{Value, json};

use super::{ROOT, TILE, child, entry_index, normalized, packed};
use crate::health_origin_assertions as proof;
use crate::health_origin_fixture::{self as fixture, Fixture, Input, TestResult};

const SECOND: &str = r"Data\ObjectContainers\PU\system\zeta\zetasystem.socpak";
const MISSING: &str = r"Data\ObjectContainers\PU\alpha\unavailable.socpak";
const INSTANCES: &str = "/placements/containerInstances/rows";
const UNRESOLVED: &str = "/placements/unresolvedContainerLinks/rows";

#[test]
fn full_container_visit_matches_normal_and_closes_sources_at_each_root() -> TestResult {
    let input = source_fixture(false)?;
    let catalog: Value = serde_json::from_slice(
        build_catalog(&input.archive, fixture::options().with_full_provenance())?.json(),
    )?;
    let mut captured = Captured::default();
    let summary = visit_container_placements_with_provenance_mode(
        &input.archive,
        full_proof::MODE,
        None,
        |event| captured.observe(&event),
    )?;
    assert!(
        summary.is_complete(),
        "a verified missing Child still has conclusive coverage"
    );
    assert!(
        summary.root_inventory.is_complete(),
        "unique original roots"
    );
    assert!(
        matches!(summary.availability, DatasetAvailability::Available),
        "both tables have an authored source family"
    );
    assert_eq!(
        summary.visited_roots, 2,
        "both canonical roots were attempted"
    );
    assert_eq!(
        summary.rows.instances, 4,
        "both roots and the repeated Tile occurrences"
    );
    assert_eq!(
        summary.rows.unresolved_links, 1,
        "the verified missing Child was retained"
    );
    let [first, second] = captured.roots.as_slice() else {
        return Err("one sealed inventory for each observed root".into());
    };
    assert_eq!(
        [first.entry, second.entry],
        [
            entry_index(&input.archive, ROOT)?,
            entry_index(&input.archive, SECOND)?
        ],
        "folder order differs from original physical Entry order"
    );
    assert_eq!(
        [first.verified_missing, second.verified_missing],
        [1, 0],
        "scope of the verified absence"
    );
    assert_table(&captured, &catalog, INSTANCES)?;
    assert_table(&captured, &catalog, UNRESOLVED)?;
    assert_other_root_not_in_inventory(first, SECOND)?;
    assert_other_root_not_in_inventory(second, ROOT)
}

#[test]
fn full_container_visit_keeps_unreadable_root_coverage_and_cancellation_distinct() -> TestResult {
    let input = source_fixture(true)?;
    let mut roots = Vec::new();
    let summary = visit_container_placements(&input.archive, None, |event| {
        if let ContainerPlacementEvent::RootCompleted(root) = event {
            roots.push((
                root.coverage.is_complete(),
                root.coverage.unreadable_containers,
            ));
        }
        Ok(())
    })?;
    assert!(
        !summary.is_complete(),
        "an unreadable target blocks full coverage"
    );
    assert_eq!(summary.rows.instances, 2, "the two readable roots remain");
    assert_eq!(
        summary.rows.unresolved_links, 3,
        "two unreadable routes and the verified missing Child"
    );
    assert_eq!(
        roots,
        [(false, 2), (true, 0)],
        "only the affected root is incomplete"
    );

    let cancel = CancelToken::new();
    let mut started = 0;
    let mut completed = 0;
    let interrupted = visit_container_placements(&input.archive, Some(&cancel), |event| {
        match event {
            ContainerPlacementEvent::Instance(_) => {
                started += 1;
                cancel.cancel();
            }
            ContainerPlacementEvent::RootCompleted(_) => completed += 1,
            _ => {}
        }
        Ok(())
    });
    assert!(
        matches!(interrupted, Err(CatalogError::Cancelled)),
        "cancellation reaches the caller"
    );
    assert_eq!(
        (started, completed),
        (1, 0),
        "the interrupted root never seals its evidence"
    );
    Ok(())
}

#[test]
fn full_container_visit_reports_ambiguous_root_without_using_a_lower_priority_path() -> TestResult {
    const ALIAS: &str = "DATA/objectcontainers/PU/system/ALPHA/ALPHASYSTEM.SOCPAK";
    const FALLBACK: &str = r"Data\ObjectContainers\PU\system\alpha\asystem.socpak";
    let input = Fixture::open(
        &fixture::empty_database(),
        Input::Plain,
        vec![
            packed(FALLBACK, "asystem.xml", b"<ObjectContainer/>"),
            packed(ROOT, "alphasystem.xml", b"<ObjectContainer/>"),
            packed(ALIAS, "ALPHASYSTEM.xml", b"<ObjectContainer/>"),
        ],
    )?;
    let mut callbacks = 0;
    let summary = visit_container_placements(&input.archive, None, |_| {
        callbacks += 1;
        Ok(())
    })?;
    assert_eq!(
        callbacks, 0,
        "the ambiguous source admits no physical root callbacks"
    );
    assert_eq!(
        summary.visited_roots, 0,
        "only uniquely selected roots are traversed"
    );
    assert!(
        !summary.is_complete(),
        "a known ambiguous root blocks conclusive coverage"
    );
    let [choice] = summary.root_inventory.roots.as_slice() else {
        return Err("the winning path still has a directory disposition".into());
    };
    assert_eq!(
        choice.status.value(),
        Some(&ContainerRootStatus::Ambiguous),
        "observed ambiguity"
    );
    assert_eq!(
        choice.candidates.len(),
        2,
        "the lower-priority alternate remains excluded"
    );
    Ok(())
}

#[derive(Default)]
struct Captured {
    instances: Vec<Value>,
    unresolved: Vec<Value>,
    roots: Vec<CapturedRoot>,
}

struct CapturedRoot {
    entry: u32,
    verified_missing: usize,
    document: Value,
}

impl Captured {
    fn observe(&mut self, event: &ContainerPlacementEvent<'_>) -> Result<(), CatalogError> {
        match event {
            ContainerPlacementEvent::Instance(row) => {
                self.instances
                    .push(serde_json::to_value(row).map_err(CatalogError::Json)?);
            }
            ContainerPlacementEvent::UnresolvedLink(row) => {
                self.unresolved
                    .push(serde_json::to_value(row).map_err(CatalogError::Json)?);
            }
            ContainerPlacementEvent::RootCompleted(root) => self.close(root)?,
            _ => {}
        }
        Ok(())
    }

    fn close(&mut self, root: &ContainerPlacementRootVisit) -> Result<(), CatalogError> {
        assert!(
            root.coverage.is_complete(),
            "a directory-proven absence permits full coverage"
        );
        assert_eq!(
            self.instances.len(),
            root.rows.instances,
            "resolved count is root-local"
        );
        assert_eq!(
            self.unresolved.len(),
            root.rows.unresolved_links,
            "unresolved count is root-local"
        );
        let sources = serde_json::to_value(&root.source_inventory).map_err(CatalogError::Json)?;
        self.roots.push(CapturedRoot {
            entry: root.root_entry_index,
            verified_missing: root.coverage.verified_missing_containers,
            document: json!({
                "placements": {
                    "containerInstances": { "rows": std::mem::take(&mut self.instances) },
                    "unresolvedContainerLinks": { "rows": std::mem::take(&mut self.unresolved) },
                },
                "provenance": sources,
            }),
        });
        Ok(())
    }
}

fn assert_table(captured: &Captured, catalog: &Value, path: &str) -> TestResult {
    let expected = proof::rows(catalog, path)?
        .iter()
        .map(|row| normalized(row, catalog))
        .collect::<TestResult<Vec<_>>>()?;
    let mut observed = Vec::new();
    for root in &captured.roots {
        for row in proof::rows(&root.document, path)? {
            assert_eq!(
                row.pointer("/instanceId/value/rootEntryIndex"),
                Some(&json!(root.entry)),
                "each row belongs to its completed root"
            );
            observed.push(normalized(row, &root.document)?);
        }
    }
    assert_eq!(
        observed, expected,
        "complete ordered Normal values and physical source evidence at {path}"
    );
    Ok(())
}

fn assert_other_root_not_in_inventory(root: &CapturedRoot, other: &str) -> TestResult {
    assert!(
        proof::rows(&root.document, "/provenance/sources")?
            .iter()
            .all(|source| source.pointer("/path/value") != Some(&json!(other))),
        "a root inventory retains no Entry observations from the other independent root"
    );
    Ok(())
}

fn source_fixture(broken_tile: bool) -> TestResult<Fixture> {
    let children = TestElement::new("ChildObjectContainers")
        .child(child(TILE, "2,0,0"))
        .child(child(TILE, "7,0,0"))
        .child(child(MISSING, "12,0,0"));
    let main = TestElement::new("ObjectContainer")
        .attribute("version", "14")
        .child(children)
        .to_xml();
    let tile = if broken_tile {
        TestEntry::stored(TILE, b"not a readable target".as_slice())
    } else {
        packed(TILE, "tile.xml", b"<ObjectContainer/>")
    };
    Fixture::open(
        &fixture::empty_database(),
        Input::Plain,
        vec![
            packed(SECOND, "zetasystem.xml", b"<ObjectContainer/>"),
            packed(ROOT, "alphasystem.xml", main.as_bytes()).encrypted(),
            tile,
        ],
    )
}
