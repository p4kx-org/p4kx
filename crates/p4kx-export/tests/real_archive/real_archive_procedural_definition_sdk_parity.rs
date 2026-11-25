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

use crate::real_archive_catalog_table_parity::{
    DefinitionOwner, StreamedDefinitionDigests, TestResult, with_bound_measurement_from_producer,
};
use crate::real_archive_scoped_equipment_sdk::equipment_input;
use crate::real_archive_verification;

const PRODUCER: &str = "P4KX_PROCEDURAL_DEFINITION_CATALOG_PRODUCER_REVISION";
const COLD_DATASET: &str = "P4KX_PROCEDURAL_DEFINITION_SDK_COLD_DATASET";
const COUNT: usize = 4;

#[derive(Clone, Copy)]
enum Family {
    ActorRecoilConfigs,
    ActorRecoilModifiers,
    WeaponAnimations,
    WeaponClips,
}

const FAMILIES: [Family; COUNT] = [
    Family::ActorRecoilConfigs,
    Family::ActorRecoilModifiers,
    Family::WeaponAnimations,
    Family::WeaponClips,
];

impl Family {
    const fn path(self) -> &'static str {
        match self {
            Self::ActorRecoilConfigs => "/definitions/actorProceduralRecoilConfigs",
            Self::ActorRecoilModifiers => "/definitions/actorProceduralRecoilModifiers",
            Self::WeaponAnimations => "/definitions/weaponProceduralAnimations",
            Self::WeaponClips => "/definitions/weaponProceduralClips",
        }
    }
}

struct Retention<'proof, 'scratch> {
    table: usize,
    proof: &'proof mut StreamedDefinitionDigests<'scratch, COUNT>,
}

#[test]
#[ignore = "installed Archive: configure the expected Build and one exact Procedural dataset"]
fn installed_procedural_definitions_cold_query() -> TestResult {
    let path = env::var(COLD_DATASET)?;
    let family = FAMILIES
        .into_iter()
        .find(|family| family.path() == path)
        .ok_or("configure one exact Actor or Weapon Procedural definition dataset path")?;
    equipment_input::with_archive(|archive| {
        let session =
            CatalogQuerySession::open(archive, CatalogLanguageSelection::Selected(&[]), None)?;
        let parsed = session.parsed(None)?;
        read_family(archive, &parsed, family, None)
    })
}

#[test]
#[ignore = "same Build: configure the installed Archive and exact completed Normal producer"]
fn installed_procedural_definitions_match_one_complete_catalog() -> TestResult {
    let producer = env::var(PRODUCER)?;
    with_bound_measurement_from_producer(&producer, |archive, measurement| {
        let scratch = tempfile::tempdir()?;
        let mut proof =
            StreamedDefinitionDigests::new(scratch.path(), [DefinitionOwner::NestedRecord; COUNT]);
        {
            let session =
                CatalogQuerySession::open(archive, CatalogLanguageSelection::Selected(&[]), None)?;
            let parsed = session.parsed(None)?;
            for (table, family) in FAMILIES.into_iter().enumerate() {
                read_family(
                    archive,
                    &parsed,
                    family,
                    Some(Retention {
                        table,
                        proof: &mut proof,
                    }),
                )?;
            }
        }
        let compared = proof.compare(measurement, FAMILIES.map(Family::path))?;
        real_archive_verification::print_line(&format!(
            "complete Procedural SDK parity: rows={:?} sourceUses={} physicalSources={}",
            compared.rows, compared.source_uses, compared.physical_sources
        ));
        Ok(())
    })
}

fn read_family(
    archive: &Archive,
    parsed: &ParsedCatalogQueries<'_>,
    family: Family,
    retention: Option<Retention<'_, '_>>,
) -> TestResult {
    let all = CatalogRecordSelection::All;
    match family {
        Family::ActorRecoilConfigs => {
            parsed.with_actor_procedural_recoil_configs(all, None, |sdk| {
                observe(archive, family, &sdk, retention)
            })?
        }
        Family::ActorRecoilModifiers => {
            parsed.with_actor_procedural_recoil_modifiers(all, None, |sdk| {
                observe(archive, family, &sdk, retention)
            })?
        }
        Family::WeaponAnimations => parsed.with_weapon_procedural_animations(all, None, |sdk| {
            observe(archive, family, &sdk, retention)
        })?,
        Family::WeaponClips => parsed.with_weapon_procedural_clips(all, None, |sdk| {
            observe(archive, family, &sdk, retention)
        })?,
    }
}

fn observe<Row: Serialize>(
    archive: &Archive,
    family: Family,
    sdk: &EquipmentSnapshot<'_, Row>,
    retention: Option<Retention<'_, '_>>,
) -> TestResult {
    let path = family.path();
    let availability = sdk.availability();
    let rows = sdk.rows();
    let coverage = sdk.coverage();
    if sdk.build().is_none()
        || sdk.build() != archive.build()
        || !known_population(availability, rows.len())
        || !coverage.unreadable_headers.is_empty()
        || !coverage.unreadable_bodies.is_empty()
        || !coverage.incomplete_fields.is_empty()
    {
        return Err(
            format!("{path}: expected complete original evidence in the matching Build").into(),
        );
    }
    let inventory = sdk.source_inventory();
    let uses = inventory.use_count();
    let sources = inventory.source_count();
    if !rows.is_empty() && (uses == 0 || sources == 0) {
        return Err(format!("{path}: the Procedural definition rows lack original sources").into());
    }
    if let Some(retention) = retention {
        retention
            .proof
            .retain(retention.table, availability, rows, inventory)?;
    }
    real_archive_verification::print_line(&format!(
        "Procedural SDK family: dataset={path} availability={availability:?} rows={} inventoryUses={uses} inventorySources={sources} skippedLeftovers={}",
        rows.len(),
        coverage.skipped_leftovers,
    ));
    Ok(())
}

const fn known_population(availability: DatasetAvailability, rows: usize) -> bool {
    match availability {
        DatasetAvailability::Available => true,
        DatasetAvailability::Absent => rows == 0,
        _ => false,
    }
}

#[test]
fn procedural_definition_parity_requires_nested_physical_record_identity() -> TestResult {
    use serde_json::json;

    let nested = json!({"source":{"recordIndex":{"state":"present","value":42}}});
    let flat = json!({"recordIndex":{"state":"present","value":42}});
    let missing = json!({"source":{"recordIndex":{"state":"absent","value":42}}});
    let owner = DefinitionOwner::NestedRecord;
    assert_eq!(
        owner.physical_index(&nested)?,
        42,
        "the original nested owner supplies the identity"
    );
    assert!(
        owner.physical_index(&flat).is_err(),
        "the comparison cannot fall back to a different header"
    );
    assert!(
        owner.physical_index(&missing).is_err(),
        "a numeric value without readable physical evidence is rejected"
    );
    assert_eq!(
        DefinitionOwner::Record.physical_index(&flat)?,
        42,
        "flat Record tables keep their own route"
    );
    Ok(())
}

#[test]
fn procedural_definition_parity_retains_declared_empty_and_absent_flows() {
    assert!(
        known_population(DatasetAvailability::Available, 0),
        "a declared empty family remains available"
    );
    assert!(
        known_population(DatasetAvailability::Absent, 0),
        "an undeclared empty family remains absent"
    );
    assert!(
        !known_population(DatasetAvailability::Absent, 1),
        "no positive row can belong to an absent family"
    );
}
