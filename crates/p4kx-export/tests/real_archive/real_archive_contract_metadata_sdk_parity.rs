// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::env;

use p4kx_export::catalog_queries::equipment::EquipmentSnapshot;
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, DatasetAvailability,
    ParsedCatalogQueries,
};
use p4kx_extraction::Archive;
use serde::Serialize;
use serde_json::Value;

use super::equipment_input;
use crate::real_archive_catalog_table_parity::{self, Origins, Table, TestResult};
use crate::real_archive_verification;

const CONTRACT_SCOPES: &str = "/definitions/contractScopes";
const MISSION_BOARDS: &str = "/configurations/missionBoards";
const DATASETS: [&str; 2] = [CONTRACT_SCOPES, MISSION_BOARDS];

#[derive(Debug, Eq, Hash, PartialEq)]
enum PhysicalRow {
    Record(u32),
    Component {
        record_index: u32,
        component_ordinal: u32,
    },
}

#[test]
#[ignore = "same Build: configure the installed Archive and exact completed Normal producer"]
fn installed_contract_metadata_matches_one_complete_catalog() -> TestResult {
    let revision = env::var("P4KX_CONTRACT_METADATA_CATALOG_PRODUCER_REVISION")
        .map_err(|_| "configure the exact contract metadata Catalog producer revision")?;
    real_archive_catalog_table_parity::with_bound_measurement_from_producer(
        &revision,
        |archive, measurement| {
            let mut origins = Origins::default();
            let tables = {
                let session = CatalogQuerySession::open(
                    archive,
                    CatalogLanguageSelection::Selected(&[]),
                    None,
                )?;
                let parsed = session.parsed(None)?;
                [
                    query(archive, &parsed, CONTRACT_SCOPES, Some(&mut origins))?
                        .ok_or("missing complete contract scope table")?,
                    query(archive, &parsed, MISSION_BOARDS, Some(&mut origins))?
                        .ok_or("missing complete mission board table")?,
                ]
            };
            let counts = measurement.verify_tables(DATASETS, &tables, &origins)?;
            real_archive_verification::print_line(&format!(
                "complete contract metadata SDK parity: rows={:?} sourceUses={} physicalSources={}",
                counts.rows, counts.source_uses, counts.physical_sources,
            ));
            Ok(())
        },
    )
}

#[test]
#[ignore = "standalone cold query: configure the installed Archive and one exact dataset path"]
fn installed_contract_metadata_cold_query() -> TestResult {
    let path = env::var("P4KX_CONTRACT_METADATA_SDK_COLD_DATASET")
        .map_err(|_| "configure one exact contract metadata dataset path")?;
    if !DATASETS.contains(&path.as_str()) {
        return Err("unknown contract metadata dataset path".into());
    }
    equipment_input::with_archive(|archive| {
        let session =
            CatalogQuerySession::open(archive, CatalogLanguageSelection::Selected(&[]), None)?;
        let parsed = session.parsed(None)?;
        let _ = query(archive, &parsed, &path, None)?;
        Ok(())
    })
}

fn query(
    archive: &Archive,
    parsed: &ParsedCatalogQueries<'_>,
    path: &str,
    origins: Option<&mut Origins>,
) -> TestResult<Option<Table>> {
    let all = CatalogRecordSelection::All;
    match path {
        CONTRACT_SCOPES => parsed.with_contract_scopes(all, None, |snapshot| {
            observe(archive, path, &snapshot, origins)
        })?,
        MISSION_BOARDS => parsed.with_mission_boards(all, None, |snapshot| {
            observe(archive, path, &snapshot, origins)
        })?,
        _ => Err("unknown contract metadata dataset path".into()),
    }
}

fn observe<Row: Serialize>(
    archive: &Archive,
    path: &str,
    sdk: &EquipmentSnapshot<'_, Row>,
    origins: Option<&mut Origins>,
) -> TestResult<Option<Table>> {
    let rows = sdk.rows();
    let availability = sdk.availability();
    let coverage = sdk.coverage();
    let evidence = match (availability, rows.len()) {
        (DatasetAvailability::Available, 0) => "complete-empty",
        (DatasetAvailability::Available, _) => "present",
        (DatasetAvailability::Absent, 0) => "absent-type",
        _ => return Err(format!("{path}: unresolved or inconsistent source availability").into()),
    };
    if sdk.build().is_none()
        || sdk.build() != archive.build()
        || !coverage.unreadable_headers.is_empty()
        || !coverage.unreadable_bodies.is_empty()
        || !coverage.incomplete_fields.is_empty()
    {
        return Err(format!(
            "{path}: expected complete original coverage in the matching Build: {coverage:?}"
        )
        .into());
    }
    let inventory = sdk.source_inventory();
    let uses = inventory.use_count();
    let sources = inventory.source_count();
    if !rows.is_empty() && (uses == 0 || sources == 0) {
        return Err(format!("{path}: physical rows have no original source inventory").into());
    }
    let retained = origins
        .map(|origins| {
            real_archive_catalog_table_parity::capture_rows(
                availability,
                rows,
                inventory,
                origins,
                |row| physical_row(path, row),
            )
        })
        .transpose()?;
    real_archive_verification::print_line(&format!(
        "Contract metadata SDK: dataset={path} availability={availability:?} rowEvidence={evidence} rows={} inventoryUses={uses} inventorySources={sources} skippedLeftovers={}",
        rows.len(),
        coverage.skipped_leftovers,
    ));
    Ok(retained)
}

fn physical_row(path: &str, row: &Value) -> TestResult<PhysicalRow> {
    let index = |key| {
        row.pointer(key)
            .and_then(Value::as_u64)
            .and_then(|value| u32::try_from(value).ok())
            .ok_or_else(|| format!("{path}: missing physical row identity at {key}"))
    };
    match path {
        CONTRACT_SCOPES => Ok(PhysicalRow::Record(index("/recordIndex/value")?)),
        MISSION_BOARDS => Ok(PhysicalRow::Component {
            record_index: index("/owner/ownerRecordIndex/value")?,
            component_ordinal: index("/owner/componentOrdinal/value")?,
        }),
        _ => Err("unknown contract metadata dataset path".into()),
    }
}

#[test]
fn contract_metadata_parity_requires_the_physical_key_for_each_family() -> TestResult {
    use serde_json::json;

    let scope = json!({"recordIndex": {"value": 7}});
    let board =
        json!({"owner": {"ownerRecordIndex": {"value": 7}, "componentOrdinal": {"value": 2}}});
    assert_eq!(
        physical_row(CONTRACT_SCOPES, &scope)?,
        PhysicalRow::Record(7),
        "scope selection uses the flattened physical Record"
    );
    assert_eq!(
        physical_row(MISSION_BOARDS, &board)?,
        PhysicalRow::Component {
            record_index: 7,
            component_ordinal: 2,
        },
        "board selection retains the physical component occurrence"
    );
    assert!(
        physical_row(MISSION_BOARDS, &scope).is_err(),
        "a scope Record without a component slot cannot witness a board"
    );
    assert!(
        physical_row(CONTRACT_SCOPES, &board).is_err(),
        "a board owner cannot stand in for an original scope Record"
    );
    Ok(())
}
