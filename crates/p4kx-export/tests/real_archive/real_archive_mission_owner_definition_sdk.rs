// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::env;

use p4kx_export::catalog_queries::mission_definitions::{
    BrokerDefinition, MissionDefinitionFamily, MissionDefinitionRows, MissionDefinitionSelection,
    MissionDefinitionSnapshot, TemplateDefinition,
};
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogRecordSelection, ParsedCatalogQueries,
};
use p4kx_extraction::Archive;
use serde::Serialize;

use super::{require_no_locales, require_record_order};
use crate::real_archive_catalog_table_parity::{
    DefinitionOwner, MissionDisposition, StreamedDefinitionDigests, TestResult,
    with_bound_measurement_from_producer,
};
use crate::real_archive_mission_configuration_sdk::parse_dispositions;
use crate::real_archive_scoped_equipment_sdk::{component_family_parity, equipment_input};
use crate::real_archive_verification::print_line;

#[path = "real_archive_mission_owner_definition_visitor_sdk.rs"]
mod visitor;

const PRODUCER: &str = "P4KX_MISSION_OWNER_DEFINITION_CATALOG_PRODUCER_REVISION";
const CENSUS: &str = "P4KX_MISSION_OWNER_DEFINITION_SDK_EXPECTED_OWNERS";
const DATASET: &str = "P4KX_MISSION_OWNER_DEFINITION_SDK_COLD_DATASET";
const COUNT: usize = 2;

#[derive(Clone, Copy)]
enum Family {
    Templates,
    Brokers,
}

const FAMILIES: [Family; COUNT] = [Family::Templates, Family::Brokers];

impl Family {
    const fn path(self) -> &'static str {
        match self {
            Self::Templates => "/definitions/contractTemplates",
            Self::Brokers => "/definitions/missionBrokerEntries",
        }
    }

    const fn selection(self) -> MissionDefinitionSelection {
        let family = match self {
            Self::Templates => MissionDefinitionFamily::ContractTemplates,
            Self::Brokers => MissionDefinitionFamily::MissionBrokerEntries,
        };
        MissionDefinitionSelection::new(family, CatalogRecordSelection::All)
    }
}

struct Retention<'proof, 'scratch> {
    table: usize,
    proof: &'proof mut StreamedDefinitionDigests<'scratch, COUNT>,
}

#[test]
#[ignore = "installed Archive: configure the Build, the sealed owner census and one dataset"]
fn installed_mission_owner_definition_cold_query() -> TestResult {
    let path = env::var(DATASET)?;
    let expected = parse_dispositions(&env::var(CENSUS)?)?;
    let (family, disposition) = FAMILIES
        .into_iter()
        .zip(expected)
        .find(|(family, _)| family.path() == path)
        .ok_or("configure one exact Template or Broker definition path")?;
    equipment_input::with_archive(|archive| {
        let session =
            equipment_input::full_source_session(archive, CatalogLanguageSelection::AllAvailable)?;
        read_family(archive, &session.parsed(None)?, family, disposition, None)
    })
}

#[test]
#[ignore = "same Build: configure the complete Normal producer and sealed owner census"]
fn installed_mission_owner_definitions_match_one_complete_catalog() -> TestResult {
    let producer = env::var(PRODUCER)?;
    let expected = parse_dispositions(&env::var(CENSUS)?)?;
    with_bound_measurement_from_producer(&producer, |archive, measurement| {
        let scratch = tempfile::tempdir()?;
        let mut proof =
            StreamedDefinitionDigests::new(scratch.path(), [DefinitionOwner::Record; COUNT]);
        {
            let session = equipment_input::full_source_session(
                archive,
                CatalogLanguageSelection::AllAvailable,
            )?;
            let parsed = session.parsed(None)?;
            for (table, (family, disposition)) in FAMILIES.into_iter().zip(expected).enumerate() {
                read_family(
                    archive,
                    &parsed,
                    family,
                    disposition,
                    Some(Retention {
                        table,
                        proof: &mut proof,
                    }),
                )?;
            }
        }
        let compared = proof.compare(measurement, FAMILIES.map(Family::path))?;
        print_line(&format!(
            "complete mission owner definition SDK parity: rows={:?} sourceUses={} physicalSources={}",
            compared.rows, compared.source_uses, compared.physical_sources
        ));
        Ok(())
    })
}

fn read_family(
    archive: &Archive,
    parsed: &ParsedCatalogQueries<'_>,
    family: Family,
    expected: MissionDisposition,
    retention: Option<Retention<'_, '_>>,
) -> TestResult {
    parsed.with_mission_definitions(family.selection(), None, |snapshot| {
        let path = family.path();
        if snapshot.build.is_none() || snapshot.build.as_ref() != archive.build() {
            return Err(format!("{path}: the query has a different Build").into());
        }
        require_no_locales(&snapshot)?;
        match (family, &snapshot.rows) {
            (Family::Templates, MissionDefinitionRows::ContractTemplates(rows)) => {
                require_record_order(rows, TemplateDefinition::header)?;
                observe(path, rows, &snapshot, expected, retention)
            }
            (Family::Brokers, MissionDefinitionRows::MissionBrokerEntries(rows)) => {
                require_record_order(rows, BrokerDefinition::header)?;
                observe(path, rows, &snapshot, expected, retention)
            }
            _ => Err(format!("{path}: the query returned the wrong mission owner family").into()),
        }
    })?
}

fn observe<Row: Serialize>(
    path: &str,
    rows: &[Row],
    snapshot: &MissionDefinitionSnapshot<'_>,
    expected: MissionDisposition,
    retention: Option<Retention<'_, '_>>,
) -> TestResult {
    let availability = snapshot.availability;
    component_family_parity::require_complete(path, availability, rows.len(), &snapshot.coverage)?;
    let observed = MissionDisposition {
        owners: rows.len(),
        skipped: snapshot.coverage.skipped_leftovers,
    };
    if observed != expected {
        return Err(format!("{path}: expected {expected:?}, observed {observed:?}").into());
    }
    let inventory = &snapshot.source_inventory;
    let uses = inventory.use_count();
    let sources = inventory.source_count();
    if !rows.is_empty() && (uses == 0 || sources == 0) {
        return Err(format!("{path}: the owner rows lack original source evidence").into());
    }
    if let Some(retention) = retention {
        retention
            .proof
            .retain(retention.table, availability, rows, inventory)?;
    }
    print_line(&format!(
        "Mission owner definitions SDK: dataset={path} availability={availability:?} rows={} inventoryUses={uses} inventorySources={sources} skippedLeftovers={} localizedRows=0",
        observed.owners, observed.skipped,
    ));
    Ok(())
}
