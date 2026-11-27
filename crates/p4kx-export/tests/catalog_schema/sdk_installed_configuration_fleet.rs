// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::CatalogError;
use p4kx_export::catalog_queries::equipment::InstalledConfigurationSnapshot;
use p4kx_export::catalog_queries::{DatasetAvailability, ParsedCatalogQueries, equipment};
use p4kx_extraction::{Archive, CancelToken, DataForgeEntry, SourceValueState};
use p4kx_testkit::{ArchiveBuilder, TestEntry};
use serde_json::Value;

use super::{
    BROKEN_SHIP_RECORD, DOCKED_RECORD, SHIP_RECORD, TestResult, check_occurrences, compare, fixture,
};

const SHIP_ALIAS_RECORD: u32 = 7;
const EMPTY_SHIP_RECORD: u32 = 8;
const UNREADABLE_SHIP_RECORD: u32 = 9;

pub(super) fn check_fleet(queries: &ParsedCatalogQueries<'_>, catalog: &Value) -> TestResult {
    let mut owners = Vec::new();
    let mut rows = Vec::new();
    let mut incomplete = 0;
    let summary = queries.visit_installed_configurations(None, |owner| -> TestResult {
        let index = owner.ship_record_index;
        owners.push(index);
        compare(owner, catalog, index)?;
        for row in &owner.rows {
            rows.push(serde_json::to_value(row)?);
        }
        incomplete += usize::from(!owner.is_complete());
        check_fleet_owner(owner)
    })?;
    assert_eq!(
        owners,
        [
            SHIP_RECORD,
            SHIP_ALIAS_RECORD,
            DOCKED_RECORD,
            BROKEN_SHIP_RECORD,
            EMPTY_SHIP_RECORD,
            UNREADABLE_SHIP_RECORD,
        ],
        "canonical GUID order retains aliases, empty owners and failed roots"
    );
    assert_eq!(
        summary.placement_count,
        rows.len(),
        "the terminal count covers every emitted placement"
    );
    assert_eq!(
        &Value::Array(rows),
        catalog
            .pointer("/placements/installedConfigurations/rows")
            .ok_or("canonical fleet rows")?,
        "all ordered rows and their per-value evidence match the producer"
    );
    assert_eq!(
        summary.owner_count,
        owners.len(),
        "even empty and unreadable roots count as visited owners"
    );
    assert_eq!(
        summary.incomplete_owner_count, incomplete,
        "the terminal count agrees with callback coverage"
    );
    assert!(
        matches!(summary.availability, DatasetAvailability::Available),
        "the authored thruster family anchors the fleet table"
    );
    Ok(())
}

fn check_fleet_owner(owner: &InstalledConfigurationSnapshot) -> TestResult {
    match owner.ship_record_index {
        SHIP_RECORD => {
            check_occurrences(owner)?;
            assert!(
                !owner.is_complete(),
                "the original dangling attachment remains visible"
            );
        }
        SHIP_ALIAS_RECORD => check_ship_alias(owner)?,
        BROKEN_SHIP_RECORD => assert!(
            !owner.is_complete(),
            "a readable flight row cannot hide its damaged sibling"
        ),
        EMPTY_SHIP_RECORD => {
            assert_eq!(
                owner.loadout_state,
                SourceValueState::Present,
                "a successfully resolved empty Loadout still exists"
            );
            assert!(
                owner.rows.is_empty() && owner.is_complete(),
                "empty resolved owner"
            );
        }
        UNREADABLE_SHIP_RECORD => {
            assert_eq!(
                owner.loadout_state,
                SourceValueState::Unreadable,
                "an admitted root with an invalid original body cannot resolve"
            );
            assert!(
                owner.rows.is_empty() && !owner.is_complete(),
                "failed physical root"
            );
            assert!(
                owner
                    .loadout_coverage
                    .incomplete_fields
                    .contains(&UNREADABLE_SHIP_RECORD),
                "the failure identifies the exact original owner"
            );
        }
        _ => {}
    }
    Ok(())
}

fn check_ship_alias(owner: &InstalledConfigurationSnapshot) -> TestResult {
    let [row] = owner.rows.as_slice() else {
        return Err("the physical Ship alias retains its own flight component".into());
    };
    assert_eq!(
        row.ship_record_index().value(),
        Some(&SHIP_ALIAS_RECORD),
        "a physical alias with the same GUID retains its own index"
    );
    assert!(
        row.position()
            .value()
            .ok_or("alias position")?
            .port_ordinals()
            .is_empty(),
        "the alias configuration belongs to its root"
    );
    Ok(())
}

#[test]
fn installed_configuration_visit_stops_on_cancellation_or_callback_failure() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("installed-flight-stop.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(
            r"Data\Game2.dcb",
            fixture::database().build(),
        ))
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, None)?;
    let entry = DataForgeEntry::read(&archive)?;
    let cancel = CancelToken::new();
    let mut observed = Vec::new();
    let result =
        equipment::visit_installed_configurations_from(&archive, &entry, Some(&cancel), |owner| {
            observed.push(owner.ship_record_index);
            cancel.cancel();
            Ok(())
        });
    assert!(matches!(result, Err(CatalogError::Cancelled)));
    assert_eq!(
        observed,
        [SHIP_RECORD],
        "cancellation stops before any later callback"
    );

    let mut observed = Vec::new();
    let result = equipment::visit_installed_configurations(&archive, None, |owner| -> TestResult {
        observed.push(owner.ship_record_index);
        Err("consumer output failed".into())
    });
    assert_eq!(
        result
            .expect_err("the callback error reaches the caller")
            .to_string(),
        "consumer output failed"
    );
    assert_eq!(
        observed,
        [SHIP_RECORD],
        "callback failure stops before any later callback"
    );
    Ok(())
}
