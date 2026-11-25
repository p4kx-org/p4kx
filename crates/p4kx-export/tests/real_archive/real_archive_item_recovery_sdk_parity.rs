// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::env;

use p4kx_export::catalog_queries::item_recovery::{ItemRecoverySnapshot, with_item_recovery};
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogRecordSelection, DatasetAvailability,
};
use p4kx_extraction::Archive;
use serde::Serialize;
use serde_json::Value;

use crate::real_archive_catalog_table_parity::{
    self, BoundMeasurement, Origins, ScopedTextProof, Table, TestResult,
};
use crate::real_archive_scoped_equipment_sdk::equipment_input;
use crate::real_archive_verification;

const DATASETS: [&str; 3] = [
    "/configurations/itemRecoveryEconomy",
    "/configurations/itemRecoveryOverrides",
    "/configurations/itemRecoveryPolicy",
];
const PRODUCER: &str = "P4KX_ITEM_RECOVERY_CATALOG_PRODUCER_REVISION";

#[test]
#[ignore = "same Build: configure the installed Archive and exact completed Normal producer"]
fn installed_item_recovery_matches_one_complete_catalog() -> TestResult {
    let producer = env::var(PRODUCER)?;
    real_archive_catalog_table_parity::with_bound_measurement_from_producer(
        &producer,
        |archive, measurement| read(archive, Some(measurement)),
    )
}

#[test]
#[ignore = "standalone cold query: configure the installed Archive and expected Build"]
fn installed_item_recovery_cold_query() -> TestResult {
    equipment_input::with_archive(|archive| read(archive, None))
}

fn read(archive: &Archive, measurement: Option<&BoundMeasurement>) -> TestResult {
    with_item_recovery(
        archive,
        CatalogRecordSelection::All,
        CatalogLanguageSelection::AllAvailable,
        None,
        |snapshot| {
            let count = validate(archive, &snapshot)?;
            let inventory = &snapshot.source_inventory;
            real_archive_verification::print_line(&format!(
                "Item Recovery SDK cohort: availability={:?} rows={count} notificationTexts={} inventoryUses={} inventorySources={} skippedLeftovers={}",
                snapshot.availability,
                snapshot.game_texts.len(),
                inventory.use_count(),
                inventory.source_count(),
                snapshot.coverage.skipped_leftovers,
            ));
            if let Some(measurement) = measurement {
                compare(&snapshot, measurement)?;
            }
            Ok(())
        },
    )?
}

fn validate(archive: &Archive, snapshot: &ItemRecoverySnapshot<'_>) -> TestResult<usize> {
    let coverage = &snapshot.coverage;
    if snapshot.build.is_none()
        || snapshot.build.as_ref() != archive.build()
        || !coverage.unreadable_headers.is_empty()
        || !coverage.unreadable_bodies.is_empty()
        || !coverage.incomplete_fields.is_empty()
    {
        return Err("Item Recovery needs complete original evidence in the matching Build".into());
    }

    let owners = [
        owner(snapshot.economy.as_ref(), |row| {
            row.source.record_index().value()
        })?,
        owner(snapshot.overrides.as_ref(), |row| {
            row.owner_record_index.value()
        })?,
        owner(snapshot.policy.as_ref(), |row| {
            row.owner_record_index.value()
        })?,
    ];

    let count = validate_owners(snapshot.availability, owners)?;
    if count == 0 {
        if !snapshot.game_texts.is_empty() {
            return Err("an empty Item Recovery scope has unexpected notification text".into());
        }
    } else if snapshot.source_inventory.use_count() == 0
        || snapshot.source_inventory.source_count() == 0
    {
        return Err("the Item Recovery owner lacks its original source closure".into());
    }
    Ok(count)
}

fn owner<Row>(
    row: Option<&Row>,
    index: impl FnOnce(&Row) -> Option<&u32>,
) -> TestResult<Option<u32>> {
    row.map(|row| {
        index(row)
            .copied()
            .ok_or_else(|| "admitted Item Recovery row lacks its Record index".into())
    })
    .transpose()
}

fn validate_owners(
    availability: DatasetAvailability,
    owners: [Option<u32>; 3],
) -> TestResult<usize> {
    match (availability, owners) {
        (DatasetAvailability::Available | DatasetAvailability::Absent, [None, None, None]) => Ok(0),
        (DatasetAvailability::Available, [Some(economy), Some(overrides), Some(policy)])
            if economy == overrides && economy == policy =>
        {
            Ok(1)
        }
        _ => Err("the three Item Recovery projections need one shared physical Record or a known empty scope".into()),
    }
}

fn compare(snapshot: &ItemRecoverySnapshot<'_>, measurement: &BoundMeasurement) -> TestResult {
    let mut origins = Origins::default();
    let tables = [
        table(
            snapshot,
            snapshot.economy.as_slice(),
            "/source/recordIndex",
            &mut origins,
        )?,
        table(
            snapshot,
            snapshot.overrides.as_slice(),
            "/ownerRecordIndex",
            &mut origins,
        )?,
        table(
            snapshot,
            snapshot.policy.as_slice(),
            "/ownerRecordIndex",
            &mut origins,
        )?,
    ];
    let mut texts = ScopedTextProof::default();
    texts.retain(
        &[] as &[Value],
        &snapshot.game_texts,
        &snapshot.source_inventory,
        &mut origins,
    )?;
    let compared =
        measurement.verify_table_scope(DATASETS, &tables, &[], &origins, Some(&texts))?;
    real_archive_verification::print_line(&format!(
        "complete Item Recovery SDK parity: rows={:?} sourceUses={} physicalSources={}",
        compared.rows, compared.source_uses, compared.physical_sources
    ));
    Ok(())
}

fn table(
    snapshot: &ItemRecoverySnapshot<'_>,
    rows: &[impl Serialize],
    key: &str,
    origins: &mut Origins,
) -> TestResult<Table> {
    real_archive_catalog_table_parity::capture_rows(
        snapshot.availability,
        rows,
        &snapshot.source_inventory,
        origins,
        |row| physical_owner(row, key),
    )
}

fn physical_owner(row: &Value, path: &str) -> TestResult<u32> {
    let header = row
        .pointer(path)
        .ok_or("missing original Item Recovery owner")?;
    if header.get("state").and_then(Value::as_str) != Some("present") {
        return Err("the original Item Recovery owner is unavailable".into());
    }
    header
        .get("value")
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .ok_or_else(|| "invalid physical Item Recovery Record index".into())
}

#[test]
fn item_recovery_parity_requires_all_projections_to_have_the_same_physical_owner() -> TestResult {
    use serde_json::json;

    let economy = json!({"source":{"recordIndex":{"state":"present","value":42}}});
    let policy = json!({"ownerRecordIndex":{"state":"present","value":42}});
    let owners = [
        Some(physical_owner(&economy, "/source/recordIndex")?),
        Some(physical_owner(&policy, "/ownerRecordIndex")?),
        Some(42),
    ];
    assert_eq!(
        validate_owners(DatasetAvailability::Available, owners)?,
        1,
        "all three rows refer to one original Record"
    );
    assert!(
        validate_owners(
            DatasetAvailability::Available,
            [Some(42), Some(43), Some(42)]
        )
        .is_err(),
        "equal nominal identifiers do not merge different physical owners"
    );
    assert!(
        validate_owners(DatasetAvailability::Available, [Some(42), None, Some(42)]).is_err(),
        "the comparison rejects a dropped projection"
    );
    assert!(
        physical_owner(&policy, "/source/recordIndex").is_err(),
        "the economy and policy use different original header positions"
    );
    Ok(())
}

#[test]
fn item_recovery_parity_retains_declared_empty_and_absent_flows() -> TestResult {
    for state in [DatasetAvailability::Available, DatasetAvailability::Absent] {
        assert_eq!(
            validate_owners(state, [None; 3])?,
            0,
            "an empty scope keeps its {state:?} status"
        );
    }
    assert!(
        validate_owners(DatasetAvailability::Absent, [Some(42); 3]).is_err(),
        "an absent source type cannot emit a positive row"
    );
    Ok(())
}
