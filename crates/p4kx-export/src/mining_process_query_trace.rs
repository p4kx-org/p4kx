// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::super::rows::{Globals, Refining};
use super::{TestResult, observe};
use crate::catalog_queries::CatalogRecordSelection;

#[path = "mining_process_query_trace_fixture.rs"]
mod fixture;

#[test]
fn selected_globals_and_refining_read_only_physical_owner_bodies() -> TestResult {
    let database = fixture::database(false);
    let globals = observe::<Globals>(
        &database,
        CatalogRecordSelection::Guid("00000000-0000-0000-0000-000000000001".parse()?),
        |_| {},
    )?;
    assert_eq!(
        globals.rows, 2,
        "every admitted same-GUID physical owner survives"
    );
    assert_eq!(
        globals.reads,
        [(0, true), (6, true)],
        "no sibling, refining, Resource or foreign bodies are read"
    );
    assert!(
        globals.dependencies.unreadable_bodies.is_empty(),
        "globals demand no group dependency"
    );
    let refining = observe::<Refining>(&database, CatalogRecordSelection::RecordIndex(2), |_| {})?;
    assert_eq!(
        refining.rows, 1,
        "physical refining selection returns one owner"
    );
    assert_eq!(
        refining.reads,
        [(2, true)],
        "refining reads no mining or Resource bodies"
    );
    Ok(())
}

#[test]
fn complete_family_selection_still_has_no_resource_or_foreign_body_scan() -> TestResult {
    let database = fixture::database(false);
    let globals = observe::<Globals>(&database, CatalogRecordSelection::All, |_| {})?;
    assert_eq!(
        globals.reads,
        [(0, true), (1, true), (6, true)],
        "only the canonical global family is materialized"
    );
    let refining = observe::<Refining>(&database, CatalogRecordSelection::All, |_| {})?;
    assert_eq!(
        refining.reads,
        [(2, true), (3, true)],
        "all refining owners use the ordinary leftover policy"
    );
    assert_eq!(
        refining.coverage.skipped_leftovers, 1,
        "the actual development owner is counted"
    );
    Ok(())
}

#[test]
fn empty_and_failed_global_selections_do_not_request_any_fallback_body() -> TestResult {
    let database = fixture::database(true);
    let empty = observe::<Globals>(&database, CatalogRecordSelection::RecordIndex(2), |_| {})?;
    assert_eq!(
        empty.rows, 0,
        "foreign family produces no fabricated global"
    );
    assert!(
        empty.reads.is_empty(),
        "foreign family selection opens no body"
    );
    let failed = observe::<Globals>(
        &database,
        CatalogRecordSelection::Guid("00000000-0000-0000-0000-000000000001".parse()?),
        |_| {},
    )?;
    assert_eq!(
        failed.rows, 0,
        "failed owners produce no invented source arena"
    );
    assert_eq!(
        failed.reads,
        [(0, false), (6, false)],
        "only failed original aliases are attempted"
    );
    assert_eq!(
        failed.coverage.unreadable_bodies,
        [0, 6],
        "body failure retains exact physical identities"
    );
    assert!(
        failed.coverage.incomplete_fields.is_empty(),
        "body and field failures remain separate"
    );
    assert!(
        failed.dependencies.unreadable_bodies.is_empty(),
        "no Resource or group fallback is attempted"
    );
    Ok(())
}
