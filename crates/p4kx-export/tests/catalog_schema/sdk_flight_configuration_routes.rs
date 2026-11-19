// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::UNIX_EPOCH;

use p4kx_export::catalog_queries::equipment::flight::with_thrusters_from;
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, ParsedCatalogQueries,
};
use p4kx_export::{CatalogOptions, CatalogPublicationMode, build_catalog_from};
use p4kx_extraction::{DataForge, DataForgeEntry, TreeBuilder};
use serde_json::Value;

use super::component_parity::parity;
use super::{Route, TestResult};

#[path = "sdk_flight_configuration_fixture.rs"]
mod fixture;

#[path = "sdk_flight_configuration_values.rs"]
mod values;

const CONTROLLERS: &str = "/configurations/flightControllers";
const IFCS: &str = "/configurations/ifcs";
const THRUSTERS: &str = "/configurations/thrusters";
const REMAINING: &str =
    "physical query and synthetic parity available; installed and complete producer parity open";
const ALL: CatalogRecordSelection = CatalogRecordSelection::All;

pub(super) fn read() -> TestResult<Vec<Route>> {
    let (_folder, archive) = fixture::archive(false)?;
    let session =
        CatalogQuerySession::open(&archive, CatalogLanguageSelection::Selected(&[]), None)?;
    let parsed = session.parsed(None)?;
    Ok(vec![
        parsed.with_flight_controllers(ALL, None, |snapshot| {
            Route::queried_rows(CONTROLLERS, snapshot.rows(), REMAINING)
        })??,
        parsed.with_ifcs(ALL, None, |snapshot| {
            Route::queried_rows(IFCS, snapshot.rows(), REMAINING)
        })??,
        parsed.with_thrusters(ALL, None, |snapshot| {
            Route::queried_rows(THRUSTERS, snapshot.rows(), REMAINING)
        })??,
    ])
}

#[test]
fn flight_configuration_queries_keep_physical_order_and_typed_values() -> TestResult<()> {
    let (_folder, archive) = fixture::archive(false)?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let guid = forge
        .record(0)
        .ok_or("first physical fixture owner")?
        .guid();
    let guid_selection = CatalogRecordSelection::Guid(guid);
    let session = CatalogQuerySession::from_entry(
        &archive,
        &entry,
        CatalogLanguageSelection::AllAvailable,
        None,
    )?;
    let parsed = session.parsed(None)?;
    parsed.with_flight_controllers(guid_selection, None, |snapshot| {
        values::controllers(&snapshot)
    })??;
    parsed.with_ifcs(guid_selection, None, |snapshot| values::ifcs(&snapshot))??;
    parsed.with_thrusters(guid_selection, None, |snapshot| {
        values::thrusters(&snapshot)
    })??;
    with_thrusters_from(
        &archive,
        &entry,
        CatalogRecordSelection::RecordIndex(1),
        None,
        |snapshot| -> TestResult<()> {
            let [row] = snapshot.rows() else {
                return Err("a physical index has only its own Components slots".into());
            };
            assert_eq!(
                row.owner().record_index().value(),
                Some(&1),
                "the alias retains its own physical Record index"
            );
            assert_eq!(
                row.owner().component_ordinal().value(),
                Some(&0),
                "the alias thruster occupies the first original Components slot"
            );
            Ok(())
        },
    )??;
    Ok(())
}

#[test]
fn flight_configuration_queries_bound_malformed_unrelated_owners() -> TestResult<()> {
    let (_folder, archive) = fixture::archive(true)?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    prove_failed_owner_sources(&forge)?;
    let guid = forge
        .record(0)
        .ok_or("first physical fixture owner")?
        .guid();
    let session = CatalogQuerySession::from_entry(
        &archive,
        &entry,
        CatalogLanguageSelection::Selected(&[]),
        None,
    )?;
    let parsed = session.parsed(None)?;
    for (selection, expected_rows) in [
        (CatalogRecordSelection::RecordIndex(0), 1),
        (CatalogRecordSelection::Guid(guid), 2),
    ] {
        parsed.with_ifcs(selection, None, |snapshot| {
            assert_eq!(
                snapshot.rows().len(),
                expected_rows,
                "the selected readable physical IFCS rows survive"
            );
            assert_eq!(
                snapshot.coverage().unreadable_headers.as_slice(),
                &[] as &[u32],
                "the unreadable unrelated header is outside this selection"
            );
            assert_eq!(
                snapshot.coverage().unreadable_bodies.as_slice(),
                &[] as &[u32],
                "the unreadable unrelated body is outside this selection"
            );
        })?;
    }
    failed_owner_coverage(&parsed)
}

fn prove_failed_owner_sources(forge: &DataForge<'_>) -> TestResult<()> {
    let owner = forge
        .record(fixture::UNREADABLE_BODY)
        .ok_or("unreadable body fixture Record")?;
    assert!(
        owner.record_path().is_ok(),
        "the malformed Components array belongs to a readable header"
    );
    let probe = forge
        .pointer_probe("Components", fixture::IFCS)
        .ok_or("authored Components probe")?;
    assert!(
        TreeBuilder::new().holds(&probe, &owner).is_err(),
        "the original unrelated pointer range is unreadable"
    );
    let header = forge
        .record(fixture::UNREADABLE_HEADER)
        .ok_or("unreadable header fixture Record")?;
    assert!(
        header.record_path().is_err(),
        "the exact original Record path cannot be read"
    );
    assert!(
        TreeBuilder::new().holds(&probe, &header)?,
        "the unreadable header still points to a valid IFCS component"
    );
    Ok(())
}

fn failed_owner_coverage(parsed: &ParsedCatalogQueries<'_>) -> TestResult<()> {
    let header = [fixture::UNREADABLE_HEADER];
    let body = [fixture::UNREADABLE_BODY];
    for (selection, rows, headers, bodies) in [
        (ALL, 2, header.as_slice(), body.as_slice()),
        (
            CatalogRecordSelection::RecordIndex(fixture::UNREADABLE_HEADER),
            0,
            header.as_slice(),
            &[],
        ),
        (
            CatalogRecordSelection::RecordIndex(fixture::UNREADABLE_BODY),
            0,
            &[],
            body.as_slice(),
        ),
    ] {
        parsed.with_ifcs(selection, None, |snapshot| {
            assert_eq!(
                snapshot.rows().len(),
                rows,
                "{selection:?} retains the readable IFCS rows"
            );
            assert_eq!(
                snapshot.coverage().unreadable_headers.as_slice(),
                headers,
                "{selection:?} retains original header failures"
            );
            assert_eq!(
                snapshot.coverage().unreadable_bodies.as_slice(),
                bodies,
                "{selection:?} retains original Components body failures"
            );
        })?;
    }
    Ok(())
}

#[test]
fn flight_configuration_queries_match_synthetic_producer_rows() -> TestResult<()> {
    let (_folder, archive) = fixture::archive(false)?;
    let entry = DataForgeEntry::read(&archive)?;
    let mut options = CatalogOptions::new(UNIX_EPOCH);
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let catalog: Value =
        serde_json::from_slice(build_catalog_from(&archive, &entry, &options)?.json())?;
    let session = CatalogQuerySession::from_entry(
        &archive,
        &entry,
        CatalogLanguageSelection::Selected(&[]),
        None,
    )?;
    let parsed = session.parsed(None)?;
    parsed.with_flight_controllers(ALL, None, |snapshot| {
        parity(CONTROLLERS, &snapshot, &catalog)
    })??;
    parsed.with_ifcs(ALL, None, |snapshot| parity(IFCS, &snapshot, &catalog))??;
    parsed.with_thrusters(ALL, None, |snapshot| parity(THRUSTERS, &snapshot, &catalog))??;
    Ok(())
}
