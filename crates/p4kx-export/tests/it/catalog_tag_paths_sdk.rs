// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::tags::{TagPathSnapshot, with_tag_paths, with_tag_paths_from};
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, DatasetAvailability,
};
use p4kx_export::{CatalogError, CatalogPublicationMode, build_catalog_from};
use p4kx_extraction::{CancelToken, DataForgeEntry};
use p4kx_testkit::DataForgeBuilder;
use serde_json::{Value, json};

use crate::catalog_tags_fixture::{self as fixture, at};
use crate::catalog_tags_states;
use crate::commodity_v3_fixture::{self as archive_fixture, DcbOrigin, TestResult};
use crate::commodity_v3_source_inventory::{body, reference_closure, uses};

const TABLE: &str = "/projections/tagPaths";

#[test]
fn tag_path_sdk_matches_the_encrypted_catalog_and_original_lineage() -> TestResult {
    let (_directory, archive) = archive_fixture::open(&fixture::graph(), DcbOrigin::Encrypted)?;
    let entry = DataForgeEntry::read(&archive)?;
    let mut options = archive_fixture::options();
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let catalog: Value =
        serde_json::from_slice(build_catalog_from(&archive, &entry, &options)?.json())?;
    with_tag_paths_from(
        &archive,
        &entry,
        CatalogRecordSelection::All,
        None,
        |snapshot| verify_graph(&snapshot, &catalog),
    )?
}

fn verify_graph(snapshot: &TagPathSnapshot<'_>, catalog: &Value) -> TestResult {
    let document = json!({
        "provenance": snapshot.source_inventory,
        "projections": {"tagPaths": {"availability": snapshot.availability, "rows": snapshot.rows}},
    });
    assert_eq!(
        document.pointer(TABLE),
        catalog.pointer(TABLE),
        "the selective graph emits the same original winner rows as the Catalog"
    );
    assert!(
        snapshot.graph_inventory_complete(),
        "all candidate families were read despite cycle cuts and dangling edges"
    );
    assert_eq!(
        snapshot.graph_issues.duplicate(),
        1,
        "one GUID alias is cut"
    );
    assert!(
        snapshot.graph_issues.repeated() > 0,
        "a repeated edge or cycle is cut"
    );
    assert!(
        snapshot.graph_issues.dangling() > 0,
        "the dangling edge is visible"
    );
    reference_closure(&document)?;
    let path = at(&document, "/projections/tagPaths/rows/1/path")?;
    assert_eq!(
        path.get("containsEncryptedSource"),
        Some(&json!(true)),
        "the selected lineage was read from encrypted DataForge"
    );
    let lineage = uses(&document, path)?;
    assert_eq!(
        lineage,
        uses(catalog, path)?,
        "the complete path's exact contributors"
    );
    assert!(
        lineage.contains(&(4, body("tags[2]"), "selector".to_owned())),
        "the selected root comes from its actual TagDatabase occurrence"
    );
    assert!(
        lineage.contains(&(0, body("children[3]"), "selector".to_owned())),
        "the first actual child occurrence selects the lineage"
    );
    assert!(
        !lineage.contains(&(0, body("children[4]"), "selector".to_owned())),
        "the repeated occurrence did not select the lineage"
    );
    Ok(())
}

#[test]
fn tag_path_sdk_selects_the_first_guid_winner_or_one_physical_path() -> TestResult {
    let (_directory, archive) = archive_fixture::open(&fixture::graph(), DcbOrigin::Plain)?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let leaf = forge.record(1).ok_or("original leaf Record")?.guid();
    let session = CatalogQuerySession::from_entry(
        &archive,
        &entry,
        CatalogLanguageSelection::AllAvailable,
        None,
    )?;
    let parsed = session.parsed(None)?;
    for selection in [
        CatalogRecordSelection::Guid(leaf),
        CatalogRecordSelection::RecordIndex(1),
    ] {
        parsed.with_tag_paths(selection, None, |snapshot| verify_leaf(&snapshot))??;
    }
    for index in [5, u32::MAX] {
        parsed.with_tag_paths(
            CatalogRecordSelection::RecordIndex(index),
            None,
            |snapshot| {
                assert!(
                    snapshot.rows.is_empty(),
                    "aliases and missing indices have no path rows"
                );
                assert!(
                    snapshot.graph_inventory_complete(),
                    "the global survey proves the absence"
                );
                assert!(
                    matches!(snapshot.availability, DatasetAvailability::Available),
                    "the declared Tag family remains available"
                );
            },
        )?;
    }
    Ok(())
}

fn verify_leaf(snapshot: &TagPathSnapshot<'_>) -> TestResult {
    let [row] = snapshot.rows.as_slice() else {
        return Err("exactly the first physical Tag path".into());
    };
    let fields = row.fields()?;
    let parent = fields.parent_header().ok_or("selected parent header")?;
    assert_eq!(
        fields.record_index().value(),
        Some(&1),
        "first physical row wins"
    );
    assert_eq!(
        fields.path().value().map(String::as_str),
        Some("Root.Same"),
        "actual path"
    );
    assert_eq!(fields.depth().value(), Some(&1), "zero-based depth");
    assert_eq!(fields.listed().value(), Some(&true), "listed root lineage");
    assert_eq!(
        parent.record_index.value(),
        Some(&0),
        "the exact linked parent"
    );
    assert_eq!(
        fields.parent_origin().contains_encrypted_source(),
        Some(false),
        "the topology input was plain"
    );
    assert_eq!(
        serde_json::to_value(fields)?,
        serde_json::to_value(row)?,
        "the typed SDK and the producer use the same projection"
    );
    Ok(())
}

#[test]
fn tag_path_sdk_distinguishes_absent_and_unreadable_graphs() -> TestResult {
    let (_directory, archive) =
        archive_fixture::open(&DataForgeBuilder::new().version(8), DcbOrigin::Plain)?;
    with_tag_paths(&archive, CatalogRecordSelection::All, None, |snapshot| {
        assert!(snapshot.rows.is_empty(), "no Tag records were declared");
        assert!(
            snapshot.graph_inventory_complete(),
            "the empty survey is complete"
        );
        assert!(
            matches!(snapshot.availability, DatasetAvailability::Absent),
            "absent source family"
        );
    })?;
    let (_broken_directory, broken) = archive_fixture::open(
        &catalog_tags_states::unreadable_tag_definition(),
        DcbOrigin::Plain,
    )?;
    with_tag_paths(
        &broken,
        CatalogRecordSelection::RecordIndex(u32::MAX),
        None,
        |snapshot| {
            assert!(
                snapshot.rows.is_empty(),
                "no row can stand in for a failed source"
            );
            assert!(
                !snapshot.graph_inventory_complete(),
                "the graph cannot prove absence"
            );
            assert_eq!(
                snapshot.graph_issues.unreadable(),
                1,
                "failure remains a graph outcome"
            );
            let [failed] = snapshot.graph_read_failures.as_slice() else {
                return Err("one exact unreadable original Record".into());
            };
            assert_eq!(
                failed.record().index(),
                0,
                "global coverage records this failed candidate"
            );
            Ok(())
        },
    )?
}

#[test]
fn tag_path_sdk_cancellation_stops_before_publishing_a_snapshot() -> TestResult {
    let (_directory, archive) = archive_fixture::open(&fixture::graph(), DcbOrigin::Plain)?;
    let token = CancelToken::new();
    token.cancel();
    let result: Result<(), _> =
        with_tag_paths(&archive, CatalogRecordSelection::All, Some(&token), |_| {
            panic!("a cancelled graph cannot publish a snapshot");
        });
    assert!(
        matches!(result, Err(CatalogError::Cancelled)),
        "typed cancellation"
    );
    Ok(())
}
