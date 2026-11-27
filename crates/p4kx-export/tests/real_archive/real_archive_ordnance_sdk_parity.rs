// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::env;

use p4kx_export::catalog_queries::equipment::EquipmentSnapshot;
use p4kx_export::catalog_queries::equipment::ordnance::{
    ExplosionDefinition, MissileConfiguration,
};
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogRecordSelection, DatasetAvailability, ParsedCatalogQueries,
};
use p4kx_extraction::Archive;
use serde::Serialize;
use serde_json::Value;

use super::component_family_parity;
use super::equipment_input;
use crate::real_archive_catalog_table_parity::{self, Origins, PhysicalSlot, Table, TestResult};
use crate::real_archive_verification::print_line;

#[derive(Clone, Copy)]
enum Family {
    Missiles,
    ExplosionDefinitions,
}

const FAMILIES: [Family; 2] = [Family::Missiles, Family::ExplosionDefinitions];

#[derive(Debug, Eq, Hash, PartialEq)]
enum RowIdentity {
    Record(u32),
    Component {
        record_index: u32,
        component_ordinal: u32,
    },
}

impl Family {
    const fn path(self) -> &'static str {
        match self {
            Self::Missiles => "/configurations/missileConfigurations",
            Self::ExplosionDefinitions => "/definitions/explosionDefinitions",
        }
    }

    fn query(
        self,
        archive: &Archive,
        parsed: &ParsedCatalogQueries<'_>,
        origins: Option<&mut Origins>,
    ) -> TestResult<Option<Table>> {
        let all = CatalogRecordSelection::All;
        match self {
            Self::Missiles => parsed.with_missile_configurations(all, None, |snapshot| {
                self.observe(archive, &snapshot, missile_order, origins)
            })?,
            Self::ExplosionDefinitions => {
                parsed.with_explosion_definitions(all, None, |snapshot| {
                    self.observe(archive, &snapshot, explosion_order, origins)
                })?
            }
        }
    }

    fn observe<Row: Serialize, Key: Ord>(
        self,
        archive: &Archive,
        snapshot: &EquipmentSnapshot<'_, Row>,
        key: impl Fn(&Row) -> TestResult<Key>,
        origins: Option<&mut Origins>,
    ) -> TestResult<Option<Table>> {
        let path = self.path();
        let build = snapshot.build();
        if build.is_none() || build != archive.build() {
            return Err(format!("{path}: ordnance query Build does not match").into());
        }
        let rows = snapshot.rows();
        let availability = snapshot.availability();
        let coverage = snapshot.coverage();
        component_family_parity::require_complete(path, availability, rows.len(), coverage)?;
        if matches!(self, Self::ExplosionDefinitions) && coverage.skipped_leftovers != 0 {
            return Err("ExplosionRecord selection cannot skip Entity leftovers".into());
        }
        require_strict_order(rows, key)?;

        let inventory = snapshot.source_inventory();
        let uses = inventory.use_count();
        let sources = inventory.source_count();
        if !rows.is_empty() && (uses == 0 || sources == 0) {
            return Err(
                format!("{path}: physical rows lack their original source inventory").into(),
            );
        }
        let retained = origins
            .map(|origins| {
                real_archive_catalog_table_parity::capture_rows(
                    availability,
                    rows,
                    inventory,
                    origins,
                    |row| self.wire_identity(row),
                )
            })
            .transpose()?;
        let evidence = match (availability, rows.is_empty()) {
            (DatasetAvailability::Available, false) => "present",
            (DatasetAvailability::Available, true) => "complete-empty",
            (DatasetAvailability::Absent, true) => "absent-type",
            _ => return Err("ordnance query has inconsistent source availability".into()),
        };
        print_line(&format!(
            "Ordnance SDK: dataset={path} availability={availability:?} rowEvidence={evidence} rows={} inventoryUses={uses} inventorySources={sources} skippedLeftovers={}",
            rows.len(),
            coverage.skipped_leftovers,
        ));
        Ok(retained)
    }

    fn wire_identity(self, row: &Value) -> TestResult<RowIdentity> {
        match self {
            Self::Missiles => Ok(RowIdentity::Component {
                record_index: present_index(row, "/ownerRecordIndex")?,
                component_ordinal: present_index(row, "/componentOrdinal")?,
            }),
            Self::ExplosionDefinitions => {
                Ok(RowIdentity::Record(present_index(row, "/recordIndex")?))
            }
        }
    }
}

fn present_index(row: &Value, path: &str) -> TestResult<u32> {
    let cell = row
        .pointer(path)
        .ok_or("missing physical ordnance identity")?;
    if cell.get("state").and_then(Value::as_str) != Some("present") {
        return Err("the physical ordnance identity is not present".into());
    }
    cell.get("value")
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .ok_or_else(|| "the physical ordnance identity is outside its index width".into())
}

fn require_strict_order<Row, Key: Ord>(
    rows: &[Row],
    key: impl Fn(&Row) -> TestResult<Key>,
) -> TestResult {
    let mut previous = None;
    for row in rows {
        let current = key(row)?;
        if previous.is_some_and(|prior| prior >= current) {
            return Err("ordnance SDK rows are duplicated or out of canonical order".into());
        }
        previous = Some(current);
    }
    Ok(())
}

fn missile_order(row: &MissileConfiguration<'_>) -> TestResult<PhysicalSlot> {
    let owner = row.owner();
    component_family_parity::physical_slot(owner.record_index(), owner.component_ordinal())
}

fn explosion_order(row: &ExplosionDefinition<'_>) -> TestResult<(String, u32)> {
    let source = row.source();
    let guid = source
        .guid()
        .value()
        .ok_or("missing explosion Record GUID")?
        .to_string();
    let index = *source
        .record_index()
        .value()
        .ok_or("missing explosion Record index")?;
    Ok((guid, index))
}

#[test]
#[ignore = "same Build: configure the installed Archive and exact completed Normal producer"]
fn installed_ordnance_matches_one_complete_catalog() -> TestResult {
    let producer = env::var("P4KX_ORDNANCE_CATALOG_PRODUCER_REVISION")?;
    real_archive_catalog_table_parity::with_bound_measurement_from_producer(
        &producer,
        |archive, measurement| {
            let mut origins = Origins::default();
            let tables = {
                let session = equipment_input::full_source_session(
                    archive,
                    CatalogLanguageSelection::Selected(&[]),
                )?;
                let parsed = session.parsed(None)?;
                let [missiles, definitions] = FAMILIES;
                [
                    missiles
                        .query(archive, &parsed, Some(&mut origins))?
                        .ok_or("missing complete missile table")?,
                    definitions
                        .query(archive, &parsed, Some(&mut origins))?
                        .ok_or("missing complete explosion definition table")?,
                ]
            };
            let counts =
                measurement.verify_tables(FAMILIES.map(Family::path), &tables, &origins)?;
            print_line(&format!(
                "complete ordnance SDK parity: rows={:?} sourceUses={} physicalSources={}",
                counts.rows, counts.source_uses, counts.physical_sources,
            ));
            Ok(())
        },
    )
}

#[test]
#[ignore = "standalone cold query: configure the installed Archive and one exact dataset path"]
fn installed_ordnance_cold_query() -> TestResult {
    let path = env::var("P4KX_ORDNANCE_SDK_COLD_DATASET")?;
    let family = FAMILIES
        .into_iter()
        .find(|family| family.path() == path)
        .ok_or("configure one exact ordnance dataset path")?;
    equipment_input::with_archive(|archive| {
        let session =
            equipment_input::full_source_session(archive, CatalogLanguageSelection::Selected(&[]))?;
        let parsed = session.parsed(None)?;
        let _ = family.query(archive, &parsed, None)?;
        Ok(())
    })
}

#[test]
fn ordnance_parity_retains_each_family_physical_key_and_checks_canonical_order() -> TestResult {
    use serde_json::json;

    let missile = json!({
        "ownerRecordIndex": {"state": "present", "value": 5},
        "componentOrdinal": {"state": "present", "value": 8},
    });
    let explosion = json!({"recordIndex": {"state": "present", "value": 9}});
    let unresolved = json!({"recordIndex": {"state": "unavailable", "value": 9}});
    assert_eq!(
        Family::Missiles.wire_identity(&missile)?,
        RowIdentity::Component {
            record_index: 5,
            component_ordinal: 8
        },
        "missiles name both physical positions",
    );
    assert_eq!(
        Family::ExplosionDefinitions.wire_identity(&explosion)?,
        RowIdentity::Record(9),
        "explosion definitions name a physical Record",
    );
    assert!(
        Family::ExplosionDefinitions
            .wire_identity(&missile)
            .is_err(),
        "component keys cannot stand in for definition roots"
    );
    assert!(
        Family::Missiles.wire_identity(&explosion).is_err(),
        "a definition Record is not a missile slot"
    );
    assert!(
        Family::ExplosionDefinitions
            .wire_identity(&unresolved)
            .is_err(),
        "the definition identity must be sourced and present"
    );

    let definitions = [("guid-a", 19), ("guid-a", 27), ("guid-b", 1)];
    require_strict_order(&definitions, |key| Ok(*key))?;
    assert!(
        require_strict_order(&[("guid-a", 27), ("guid-a", 19)], |key| Ok(*key)).is_err(),
        "equal GUIDs retain a strictly increasing physical Record tie-breaker",
    );
    Ok(())
}
