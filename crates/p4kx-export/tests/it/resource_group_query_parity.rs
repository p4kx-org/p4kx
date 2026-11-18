// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, DatasetAvailability,
    ResourceGroupParentSnapshot, ResourceGroupSnapshot,
};
use p4kx_export::{CatalogPublicationMode, build_catalog};
use p4kx_extraction::{Archive, DataForgeEntry};
use serde_json::Value;

use super::{fixture, texts, walk};
use crate::canonical_header::json::assert_canonical;
use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};

#[test]
fn group_queries_match_all_canonical_getters_and_origin_states() -> TestResult {
    for (origin, marker) in [
        (DcbOrigin::Plain, Some(false)),
        (DcbOrigin::Encrypted, Some(true)),
        (DcbOrigin::Unknown, None),
    ] {
        let (_folder, archive) = common::open(&fixture::graph(), origin)?;
        let catalog = catalog(&archive, CatalogPublicationMode::DiagnosticPartial)?;
        let entry = DataForgeEntry::read(&archive)?;
        let session = CatalogQuerySession::from_entry(
            &archive,
            &entry,
            CatalogLanguageSelection::AllAvailable,
            None,
        )?;
        let parsed = session.parsed(None)?;
        parsed.with_resource_groups(CatalogRecordSelection::All, None, |snapshot| {
            direct(&snapshot, &catalog)?;
            texts::verify(&snapshot, &catalog, marker)
        })??;
        parsed.with_resource_group_parents(CatalogRecordSelection::All, None, |snapshot| {
            parents(&snapshot, &catalog)
        })??;
    }
    Ok(())
}

#[test]
fn clean_normal_catalog_and_session_queries_share_canonical_group_tables() -> TestResult {
    let (_folder, archive) = common::open(&fixture::clean(), DcbOrigin::Plain)?;
    let catalog = catalog(&archive, CatalogPublicationMode::Normal)?;
    assert_eq!(
        catalog
            .pointer("/exportHealth/status")
            .and_then(Value::as_str),
        Some("complete"),
        "the positive counterpart uses normal complete publication"
    );
    let session =
        CatalogQuerySession::open(&archive, CatalogLanguageSelection::AllAvailable, None)?;
    session.with_resource_groups(CatalogRecordSelection::All, None, |snapshot| {
        direct(&snapshot, &catalog)
    })??;
    session.with_resource_group_parents(CatalogRecordSelection::All, None, |snapshot| {
        parents(&snapshot, &catalog)
    })??;
    Ok(())
}

pub(super) fn catalog(archive: &Archive, mode: CatalogPublicationMode) -> TestResult<Value> {
    let mut options = common::options();
    options.publication_mode = mode;
    Ok(serde_json::from_slice(
        build_catalog(archive, &options)?.json(),
    )?)
}

fn direct(snapshot: &ResourceGroupSnapshot<'_>, catalog: &Value) -> TestResult {
    let definitions = fixture::rows(catalog, "definitions/resourceGroups")?;
    let declarations = fixture::rows(catalog, "declarations/resourceGroupMembers")?;
    assert_eq!(
        snapshot.definitions().len(),
        definitions.len(),
        "all physical definitions survive"
    );
    assert_eq!(
        snapshot.members().len(),
        declarations.len(),
        "all physical declarations survive"
    );
    assert_canonical(
        snapshot.definitions(),
        &Value::Array(definitions.to_vec()),
        "identical canonical definitions",
    )?;
    assert_canonical(
        snapshot.members(),
        &Value::Array(declarations.to_vec()),
        "identical canonical memberships",
    )?;
    for (row, expected) in snapshot.definitions().iter().zip(definitions) {
        walk::definition(row, snapshot.source_inventory(), expected)?;
    }
    for (row, expected) in snapshot.members().iter().zip(declarations) {
        walk::members(row, snapshot.source_inventory(), expected)?;
    }
    assert!(
        snapshot.build().is_none(),
        "the synthetic source has no invented Build"
    );
    assert!(
        matches!(snapshot.availability(), DatasetAvailability::Available),
        "the actual source family is declared"
    );
    assert!(
        snapshot.coverage().unreadable_headers.is_empty(),
        "graph headers are readable"
    );
    assert!(
        snapshot.coverage().unreadable_bodies.is_empty(),
        "graph owner bodies are readable"
    );
    Ok(())
}

fn parents(snapshot: &ResourceGroupParentSnapshot, catalog: &Value) -> TestResult {
    let expected = fixture::rows(catalog, "projections/resourceGroupParents")?;
    assert_eq!(
        snapshot.rows().len(),
        expected.len(),
        "all physical parent choices survive"
    );
    assert_canonical(
        snapshot.rows(),
        &Value::Array(expected.to_vec()),
        "identical canonical parent choices",
    )?;
    for (row, expected) in snapshot.rows().iter().zip(expected) {
        walk::parent(row, snapshot.source_inventory(), expected)?;
    }
    assert!(
        snapshot.build().is_none(),
        "no synthetic installed Build is invented"
    );
    assert!(
        matches!(snapshot.availability(), DatasetAvailability::Available),
        "the actual parent source family is declared"
    );
    assert!(
        snapshot.coverage().unreadable_bodies.is_empty(),
        "selected owners are readable"
    );
    assert!(
        snapshot.dependency_coverage().unreadable_headers.is_empty(),
        "all required family headers are readable"
    );
    assert!(
        snapshot.dependency_coverage().unreadable_bodies.is_empty(),
        "the required family has no body failure"
    );
    Ok(())
}
