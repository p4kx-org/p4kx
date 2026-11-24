// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::env;

use p4kx_export::catalog_queries::equipment::EquipmentSnapshot;
use p4kx_export::catalog_queries::equipment::component_definitions::ComponentDefinitionSnapshot;
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

#[derive(Clone, Copy)]
enum Family {
    PointConfigurations,
    GlobalConfigurations,
    Drives,
    FlightTuning,
    TunnelForces,
}

const FAMILIES: [Family; 5] = [
    Family::PointConfigurations,
    Family::GlobalConfigurations,
    Family::Drives,
    Family::FlightTuning,
    Family::TunnelForces,
];

impl Family {
    const fn path(self) -> &'static str {
        match self {
            Self::PointConfigurations => "/configurations/jumpPointConfigurations",
            Self::GlobalConfigurations => "/configurations/globalJumpPointConfigurations",
            Self::Drives => "/configurations/jumpDrives",
            Self::FlightTuning => "/configurations/jumpDriveFlightTuning",
            Self::TunnelForces => "/configurations/jumpTunnelForces",
        }
    }

    fn physical_row(self, row: &Value) -> TestResult<PhysicalRow> {
        let read = |path| {
            row.pointer(path)
                .and_then(Value::as_u64)
                .and_then(|value| u32::try_from(value).ok())
                .ok_or_else(|| format!("{}: missing physical row identity {path}", self.path()))
        };
        Ok(match self {
            Self::PointConfigurations | Self::Drives => PhysicalRow::Component {
                record: read("/ownerRecordIndex/value")?,
                ordinal: read("/componentOrdinal/value")?,
            },
            Self::GlobalConfigurations => PhysicalRow::Record(read("/recordIndex/value")?),
            Self::FlightTuning | Self::TunnelForces => {
                PhysicalRow::Record(read("/source/recordIndex/value")?)
            }
        })
    }
}

#[derive(Debug, PartialEq, Eq, Hash)]
enum PhysicalRow {
    Record(u32),
    Component { record: u32, ordinal: u32 },
}

enum Evidence<'a> {
    Cold,
    Compare(&'a mut Origins),
}

#[test]
#[ignore = "same Build: configure the installed Archive and exact completed Normal producer"]
fn installed_jump_configurations_match_one_complete_catalog() -> TestResult {
    let revision = env::var("P4KX_JUMP_CONFIGURATION_CATALOG_PRODUCER_REVISION")
        .map_err(|_| "configure the exact Jump configuration Catalog producer revision")?;
    real_archive_catalog_table_parity::with_bound_measurement_from_producer(
        &revision,
        |archive, measurement| {
            let session =
                CatalogQuerySession::open(archive, CatalogLanguageSelection::Selected(&[]), None)?;
            let parsed = session.parsed(None)?;
            let mut origins = Origins::default();
            let mut tables = Vec::with_capacity(FAMILIES.len());
            for family in FAMILIES {
                let table = read_family(archive, &parsed, family, Evidence::Compare(&mut origins))?
                    .ok_or("expected a retained Jump configuration table")?;
                tables.push(table);
            }
            let tables: [Table; FAMILIES.len()] = tables
                .try_into()
                .map_err(|_| "expected all five Jump configuration tables")?;
            let compared =
                measurement.verify_tables(FAMILIES.map(Family::path), &tables, &origins)?;
            real_archive_verification::print_line(&format!(
                "complete Jump configuration SDK parity: rows={:?} sourceUses={} physicalSources={}",
                compared.rows, compared.source_uses, compared.physical_sources,
            ));
            Ok(())
        },
    )
}

#[test]
#[ignore = "installed Archive: configure the expected Build and one exact dataset path"]
fn installed_jump_configuration_cold_query() -> TestResult {
    let path = env::var("P4KX_JUMP_CONFIGURATION_SDK_COLD_DATASET")
        .map_err(|_| "configure one canonical Jump configuration dataset path")?;
    let family = FAMILIES
        .into_iter()
        .find(|family| family.path() == path)
        .ok_or("expected one exact Jump configuration dataset path")?;
    equipment_input::with_archive(|archive| {
        let session =
            CatalogQuerySession::open(archive, CatalogLanguageSelection::Selected(&[]), None)?;
        let parsed = session.parsed(None)?;
        read_family(archive, &parsed, family, Evidence::Cold)?;
        Ok(())
    })
}

fn read_family(
    archive: &Archive,
    parsed: &ParsedCatalogQueries<'_>,
    family: Family,
    evidence: Evidence<'_>,
) -> TestResult<Option<Table>> {
    let all = CatalogRecordSelection::All;
    match family {
        Family::PointConfigurations => parsed.with_jump_point_configurations(all, None, |sdk| {
            observe(archive, family, &sdk, evidence)
        })?,
        Family::GlobalConfigurations => {
            parsed.with_global_jump_point_configurations(all, None, |sdk| {
                observe_definition(archive, family, &sdk, evidence)
            })?
        }
        Family::Drives => {
            parsed.with_jump_drives(all, None, |sdk| observe(archive, family, &sdk, evidence))?
        }
        Family::FlightTuning => parsed.with_jump_drive_flight_tuning(all, None, |sdk| {
            observe_definition(archive, family, &sdk, evidence)
        })?,
        Family::TunnelForces => parsed.with_jump_tunnel_forces(all, None, |sdk| {
            observe_definition(archive, family, &sdk, evidence)
        })?,
    }
}

fn observe_definition<Row: Serialize>(
    archive: &Archive,
    family: Family,
    sdk: &ComponentDefinitionSnapshot<'_, Row>,
    evidence: Evidence<'_>,
) -> TestResult<Option<Table>> {
    let discovery = sdk.definition_discovery();
    if !discovery.is_complete() {
        return Err(format!(
            "{}: incomplete Jump definition discovery (directoryComplete={} failedRecords={} failedEntries={} incompleteRecords={} incompleteFields={})",
            family.path(),
            discovery.directory_complete(),
            discovery.failed_records().len(),
            discovery.failed_entries().len(),
            discovery.incomplete_records().len(),
            discovery.incomplete_fields(),
        ).into());
    }
    observe(archive, family, sdk, evidence)
}

fn observe<Row: Serialize, Dependencies>(
    archive: &Archive,
    family: Family,
    sdk: &EquipmentSnapshot<'_, Row, Dependencies>,
    evidence: Evidence<'_>,
) -> TestResult<Option<Table>> {
    let path = family.path();
    let availability = sdk.availability();
    let rows = sdk.rows();
    let coverage = sdk.coverage();
    let known_disposition = match availability {
        DatasetAvailability::Available => true,
        DatasetAvailability::Absent => rows.is_empty(),
        _ => false,
    };
    if sdk.build().is_none()
        || sdk.build() != archive.build()
        || !known_disposition
        || !coverage.unreadable_headers.is_empty()
        || !coverage.unreadable_bodies.is_empty()
        || !coverage.incomplete_fields.is_empty()
    {
        return Err(format!(
            "{path}: expected complete Jump configuration coverage in the matching Build"
        )
        .into());
    }

    real_archive_verification::print_line(&format!(
        "Jump configuration SDK family: dataset={path} availability={availability:?} rows={} skippedLeftovers={}",
        rows.len(),
        coverage.skipped_leftovers,
    ));
    let inventory = sdk.source_inventory();
    match evidence {
        Evidence::Compare(origins) => real_archive_catalog_table_parity::capture_rows(
            availability,
            rows,
            inventory,
            origins,
            |row| family.physical_row(row),
        )
        .map(Some),
        Evidence::Cold => {
            let uses = inventory.use_count();
            let sources = inventory.source_count();
            if !rows.is_empty() && (uses == 0 || sources == 0) {
                return Err(format!(
                    "{path}: Jump configuration rows have an empty source inventory"
                )
                .into());
            }
            real_archive_verification::print_line(&format!(
                "Jump configuration SDK cold: dataset={path} rows={} inventoryUses={uses} inventorySources={sources}",
                rows.len(),
            ));
            Ok(None)
        }
    }
}

#[test]
fn jump_configuration_parity_requires_the_exact_physical_key() -> TestResult {
    use serde_json::json;

    let component = json!({
        "ownerRecordIndex": {"value": 48},
        "componentOrdinal": {"value": 2}
    });
    let global = json!({"recordIndex": {"value": 48}});
    let definition = json!({"source": {"recordIndex": {"value": 48}}});
    for family in [Family::PointConfigurations, Family::Drives] {
        assert_eq!(
            family.physical_row(&component)?,
            PhysicalRow::Component {
                record: 48,
                ordinal: 2,
            },
            "{} must distinguish slots of the same physical owner",
            family.path(),
        );
        assert!(
            family.physical_row(&global).is_err(),
            "{} cannot identify a component by Record index alone",
            family.path(),
        );
    }
    assert_eq!(
        Family::GlobalConfigurations.physical_row(&global)?,
        PhysicalRow::Record(48),
        "global configurations use their flattened Record identity",
    );
    for family in [Family::FlightTuning, Family::TunnelForces] {
        assert_eq!(
            family.physical_row(&definition)?,
            PhysicalRow::Record(48),
            "{} uses its nested definition Record identity",
            family.path(),
        );
        assert!(
            family.physical_row(&global).is_err(),
            "{} cannot read another route's flattened identity",
            family.path(),
        );
    }
    Ok(())
}
