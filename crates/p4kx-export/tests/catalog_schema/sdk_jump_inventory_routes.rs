// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::UNIX_EPOCH;

use p4kx_export::catalog_queries::starmap::{
    DatasetAvailability, JumpInventorySnapshot, read_jump_inventory,
};
use p4kx_export::{CatalogOptions, CatalogPublicationMode, StarmapTraversalBudget, build_catalog};
use p4kx_extraction::{Archive, EntryIssue, SourceValueState};
use serde_json::{Value, json};

use super::{Route, TestResult};

const ENDPOINTS: &str = "/placements/jumpEndpoints";
const CONNECTIONS: &str = "/projections/jumpConnections";
#[path = "sdk_jump_inventory_fixture.rs"]
mod fixture;
#[path = "sdk_jump_invalid_targets.rs"]
mod invalid_targets;
#[path = "sdk_jump_inventory_provenance.rs"]
mod provenance;

use fixture::{Fixture, open_fixture, open_recovered};

pub(super) fn read() -> TestResult<[Route; 2]> {
    let (_directory, archive) = open_fixture(Fixture::Complete)?;
    let complete = verify_parity(&archive)?;
    verify_complete(&complete)?;
    verify_incomplete_fixtures()?;
    verify_recovered()?;
    verify_healthy_empty()?;

    Ok([
        Route::queried_rows(
            ENDPOINTS,
            &complete.endpoints,
            "synthetic global occurrence and source parity available; installed producer parity and SDK-only peak open",
        )?,
        Route::queried_uninhabited_rows(
            CONNECTIONS,
            complete.connections.rows(),
            "synthetic audited absence and unavailable controls available; installed global SDK parity open",
        )?,
    ])
}

fn verify_complete(complete: &JumpInventorySnapshot) -> TestResult<()> {
    assert_eq!(
        complete.coverage.systems, 2,
        "both canonical roots were considered"
    );
    assert!(
        complete.coverage.endpoint_inventory_complete,
        "the reachable endpoint inventory is complete"
    );
    assert_eq!(
        complete.coverage.incomplete_partner_occurrences, 0,
        "all observed endpoints have readable partner scope"
    );
    assert!(
        matches!(
            complete.connections.availability(),
            DatasetAvailability::Absent
        ),
        "a complete survey establishes the authored relationship inventory is empty"
    );
    let [first, repeated, second] = complete.endpoints.as_slice() else {
        return Err("two repeated alpha occurrences and one beta occurrence".into());
    };
    let first_key = first.instance_id().value().ok_or("first physical route")?;
    let repeated_key = repeated
        .instance_id()
        .value()
        .ok_or("repeated physical route")?;
    let second_key = second
        .instance_id()
        .value()
        .ok_or("second physical root's route")?;
    assert_eq!(
        first_key.root_entry_index(),
        repeated_key.root_entry_index(),
        "repeated placements share their physical root"
    );
    assert_ne!(
        first_key, repeated_key,
        "repeated placements have distinct Child routes"
    );
    assert_ne!(
        first_key.root_entry_index(),
        second_key.root_entry_index(),
        "the beta placement has its own physical root"
    );
    assert_ne!(
        first.starmap_record_index().value(),
        second.starmap_record_index().value(),
        "the beta occurrence names a different original Record"
    );
    assert_eq!(
        first.declared_partner().state(),
        SourceValueState::Absent,
        "the readable gate has no authored partner"
    );
    Ok(())
}

fn verify_recovered() -> TestResult<()> {
    let (_directory, recovered) = open_recovered()?;
    assert!(
        recovered
            .issues()
            .iter()
            .any(|(_, issue)| matches!(issue, EntryIssue::Recovered)),
        "the fixture actually recovered an incomplete directory"
    );
    let unknown = verify_parity(&recovered)?;
    assert!(
        !unknown.coverage.directory_complete,
        "a recovered directory cannot establish the root set"
    );
    assert!(
        unknown.endpoints.is_empty(),
        "the removed root supplies no observed endpoint"
    );
    assert!(
        matches!(
            unknown.connections.availability(),
            DatasetAvailability::Unavailable
        ),
        "an empty recovered result cannot certify relationship absence"
    );
    Ok(())
}

fn verify_healthy_empty() -> TestResult<()> {
    for (fixture, roots) in [(Fixture::NoRoots, 0), (Fixture::NoEndpoints, 1)] {
        let (_directory, archive) = open_fixture(fixture)?;
        let query = verify_parity(&archive)?;
        assert_eq!(
            query.coverage.systems, roots,
            "the healthy root count is exact"
        );
        assert!(
            query.coverage.endpoint_inventory_complete,
            "a healthy empty traversal has complete coverage"
        );
        assert!(
            query.endpoints.is_empty(),
            "the source supplies no admitted endpoint occurrence"
        );
        assert!(
            matches!(
                (roots, query.endpoint_availability),
                (0, DatasetAvailability::Absent) | (1, DatasetAvailability::Available)
            ),
            "availability preserves whether a canonical root was admitted"
        );
        assert!(
            matches!(
                query.connections.availability(),
                DatasetAvailability::Absent
            ),
            "a healthy empty inventory certifies scoped relationship absence"
        );
    }
    Ok(())
}

fn verify_incomplete_fixtures() -> TestResult<()> {
    for fixture in [
        Fixture::UnreadableBeta,
        Fixture::DuplicateTarget,
        Fixture::UnreadableRecordType,
    ] {
        let (_directory, archive) = open_fixture(fixture)?;
        let partial = verify_parity(&archive)?;
        assert!(
            !partial.coverage.endpoint_inventory_complete,
            "the damaged source leaves an incomplete endpoint inventory"
        );
        let Some(first) = partial.endpoints.first() else {
            return Err("an independent root retains observed endpoints".into());
        };
        assert!(
            matches!(
                partial.connections.availability(),
                DatasetAvailability::Unavailable
            ),
            "incomplete endpoint or partner evidence prevents a relationship result"
        );
        assert_eq!(
            partial.coverage.record_classifications_complete,
            !matches!(fixture, Fixture::UnreadableRecordType),
            "the unreadable admitted Record type is distinguishable from route failures"
        );
        if matches!(fixture, Fixture::DuplicateTarget) {
            let row = serde_json::to_value(first)?;
            assert_eq!(
                row.pointer("/gate/state"),
                Some(&json!("invalid")),
                "physical ambiguity prevents selecting one gate"
            );
            let candidates = row
                .pointer("/targetCandidates/value")
                .and_then(Value::as_array)
                .ok_or("both physical target candidates")?;
            let [left, right] = candidates.as_slice() else {
                return Err("expected exactly two physical candidate Entries".into());
            };
            assert_ne!(
                left.pointer("/value/entryIndex"),
                right.pointer("/value/entryIndex"),
                "the candidates retain distinct physical Entries"
            );
        }
    }
    Ok(())
}

fn verify_parity(archive: &Archive) -> TestResult<JumpInventorySnapshot> {
    let options = CatalogOptions {
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(UNIX_EPOCH)
    };
    let canonical: Value = serde_json::from_slice(build_catalog(archive, &options)?.json())?;
    let query = read_jump_inventory(archive, StarmapTraversalBudget::new(), None)?;
    let endpoints = json!({"availability": query.endpoint_availability, "rows": query.endpoints});
    assert_eq!(
        canonical.pointer(ENDPOINTS),
        Some(&endpoints),
        "the endpoint rows and availability retain canonical values"
    );
    provenance::verify(&canonical, &endpoints, &query.source_inventory)?;
    assert!(
        query.connections.rows().is_empty(),
        "the unsupported authored relationship row type is uninhabited"
    );
    assert_eq!(
        canonical.pointer(CONNECTIONS),
        Some(&serde_json::to_value(&query.connections)?),
        "the relationship availability and empty rows match the producer"
    );
    Ok(query)
}
