// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;
use std::env;

use p4kx_export::catalog_queries::mission_definitions::{
    MissionDefinitionFamily, MissionDefinitionRows, MissionDefinitionSelection,
    MissionDefinitionSnapshot, MissionItemDefinition, MissionModuleDeclaration,
    MissionScenarioDefinition,
};
use p4kx_export::catalog_queries::{
    CanonicalRecordHeader, CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection,
    DatasetAvailability, ParsedCatalogQueries,
};
use p4kx_extraction::Archive;
use serde::Serialize;
use serde_json::Value;

use crate::real_archive_catalog_table_parity::{self, Origins, Table, TestResult};
use crate::real_archive_scoped_equipment_sdk::{component_family_parity, equipment_input};
use crate::real_archive_verification::print_line;

#[path = "real_archive_mission_owner_definition_sdk.rs"]
mod owner_definitions;

#[derive(Clone, Copy)]
enum Family {
    Scenarios,
    Modules,
    Items,
}

const FAMILIES: [Family; 3] = [Family::Scenarios, Family::Modules, Family::Items];

impl Family {
    const fn path(self) -> &'static str {
        match self {
            Self::Scenarios => "/definitions/missionScenarios",
            Self::Modules => "/definitions/missionModuleDeclarations",
            Self::Items => "/definitions/missionItems",
        }
    }

    const fn selected_family(self) -> MissionDefinitionFamily {
        match self {
            Self::Scenarios => MissionDefinitionFamily::Scenarios,
            Self::Modules => MissionDefinitionFamily::Modules,
            Self::Items => MissionDefinitionFamily::Items,
        }
    }

    fn query(
        self,
        archive: &Archive,
        parsed: &ParsedCatalogQueries<'_>,
        origins: Option<&mut Origins>,
    ) -> TestResult<Option<Table>> {
        let all =
            MissionDefinitionSelection::new(self.selected_family(), CatalogRecordSelection::All);
        parsed.with_mission_definitions(all, None, |snapshot| {
            if snapshot.build.is_none() || snapshot.build.as_ref() != archive.build() {
                return Err("mission definition query has a different Build".into());
            }
            match (self, &snapshot.rows) {
                (Self::Scenarios, MissionDefinitionRows::Scenarios(rows)) => {
                    require_no_locales(&snapshot)?;
                    self.observe(&snapshot, rows, MissionScenarioDefinition::header, origins)
                }
                (Self::Modules, MissionDefinitionRows::Modules(rows)) => {
                    require_no_locales(&snapshot)?;
                    self.observe(&snapshot, rows, MissionModuleDeclaration::header, origins)
                }
                (Self::Items, MissionDefinitionRows::Items(rows)) => {
                    require_item_text_closure(&snapshot, rows)?;
                    self.observe(&snapshot, rows, MissionItemDefinition::header, origins)
                }
                _ => Err("mission definition query returned a different selected family".into()),
            }
        })?
    }

    fn observe<'source, Row: Serialize>(
        self,
        snapshot: &MissionDefinitionSnapshot<'source>,
        rows: &[Row],
        header: impl for<'row> Fn(&'row Row) -> &'row CanonicalRecordHeader<'source>,
        origins: Option<&mut Origins>,
    ) -> TestResult<Option<Table>> {
        let path = self.path();
        let availability = snapshot.availability;
        let coverage = &snapshot.coverage;
        component_family_parity::require_complete(path, availability, rows.len(), coverage)?;
        if coverage.skipped_leftovers != 0 {
            return Err(format!("{path}: unexpected mission definition policy exclusions").into());
        }
        require_record_order(rows, header)?;

        let inventory = &snapshot.source_inventory;
        let uses = inventory.use_count();
        let sources = inventory.source_count();
        if !rows.is_empty() && (uses == 0 || sources == 0) {
            return Err(format!("{path}: definition rows lack original source evidence").into());
        }
        let table = origins
            .map(|origins| {
                real_archive_catalog_table_parity::capture_rows(
                    availability,
                    rows,
                    inventory,
                    origins,
                    wire_record_index,
                )
            })
            .transpose()?;
        let state = match (availability, rows.is_empty()) {
            (DatasetAvailability::Available, false) => "present",
            (DatasetAvailability::Available, true) => "complete-empty",
            (DatasetAvailability::Absent, true) => "absent-type",
            _ => return Err("unresolved mission definition availability".into()),
        };
        print_line(&format!(
            "Mission definitions SDK: dataset={path} availability={availability:?} rowEvidence={state} rows={} inventoryUses={uses} inventorySources={sources} skippedLeftovers={}",
            rows.len(),
            coverage.skipped_leftovers,
        ));
        Ok(table)
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct DefinitionPosition {
    guid: String,
    record_index: u32,
}

#[derive(Default)]
struct DefinitionOrder {
    previous: Option<DefinitionPosition>,
    indices: HashSet<u32>,
}

impl DefinitionOrder {
    fn observe(&mut self, header: &CanonicalRecordHeader<'_>) -> TestResult {
        let current = DefinitionPosition {
            guid: header
                .guid()
                .value()
                .ok_or("missing mission definition GUID")?
                .to_string(),
            record_index: *header
                .record_index()
                .value()
                .ok_or("missing mission definition Record index")?,
        };
        if !self.indices.insert(current.record_index)
            || self
                .previous
                .as_ref()
                .is_some_and(|previous| previous >= &current)
        {
            return Err("mission definitions repeat a physical Record or differ from canonical GUID and Record order".into());
        }
        self.previous = Some(current);
        Ok(())
    }
}

fn require_record_order<'source, Row>(
    rows: &[Row],
    header: impl for<'row> Fn(&'row Row) -> &'row CanonicalRecordHeader<'source>,
) -> TestResult {
    let mut order = DefinitionOrder::default();
    for row in rows {
        order.observe(header(row))?;
    }
    Ok(())
}

fn wire_record_index(row: &Value) -> TestResult<u32> {
    let index = row
        .get("recordIndex")
        .ok_or("missing mission definition Record index")?;
    if index.get("state").and_then(Value::as_str) != Some("present") {
        return Err("mission definition Record index must be present".into());
    }
    index
        .get("value")
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .ok_or_else(|| "mission definition Record index exceeds its wire width".into())
}

fn require_no_locales(snapshot: &MissionDefinitionSnapshot<'_>) -> TestResult {
    if !snapshot.available_languages.is_empty() || !snapshot.game_texts.is_empty() {
        return Err("nonlocalized mission definitions retained localized output".into());
    }
    Ok(())
}

fn require_item_text_closure(
    snapshot: &MissionDefinitionSnapshot<'_>,
    rows: &[MissionItemDefinition<'_>],
) -> TestResult {
    if snapshot.requested_languages.is_empty()
        || snapshot
            .available_languages
            .iter()
            .any(|language| !snapshot.requested_languages.contains(language))
    {
        return Err("mission items were not read with the Normal locale policy".into());
    }
    let mut selected = HashSet::new();
    for row in rows {
        let Some(variants) = row
            .string_variants()
            .value()
            .and_then(|value| value.variants().value())
        else {
            continue;
        };
        for variant in variants.iter().filter_map(|variant| variant.value()) {
            if let Some(reference) = variant.string().value() {
                selected.insert(reference.as_str());
            }
        }
    }

    let mut returned = HashSet::new();
    let mut text_rows = Vec::new();
    for text in &snapshot.game_texts {
        let id = text
            .id()
            .value()
            .ok_or("missing protected mission text identity")?;
        if text.key().value().is_none() || !returned.insert(id.as_str()) {
            return Err(
                "the protected mission text inventory has a missing or duplicate declaration"
                    .into(),
            );
        }
        text_rows.push(real_archive_catalog_table_parity::wire_row(text)?);
    }
    if selected != returned {
        return Err(
            "the selected mission item references and local protected texts do not close".into(),
        );
    }
    Origins::default().extend(&snapshot.source_inventory, &text_rows)
}

#[test]
#[ignore = "same Build: configure the installed Archive and exact completed Normal producer"]
fn installed_mission_definitions_match_one_complete_catalog() -> TestResult {
    let revision = env::var("P4KX_MISSION_DEFINITION_CATALOG_PRODUCER_REVISION")?;
    real_archive_catalog_table_parity::with_bound_measurement_from_producer(
        &revision,
        |archive, measurement| {
            let mut origins = Origins::default();
            let tables = {
                let session = CatalogQuerySession::open(
                    archive,
                    CatalogLanguageSelection::AllAvailable,
                    None,
                )?;
                let parsed = session.parsed(None)?;
                let [scenarios, modules, items] = FAMILIES;
                [
                    scenarios
                        .query(archive, &parsed, Some(&mut origins))?
                        .ok_or("missing complete scenario table")?,
                    modules
                        .query(archive, &parsed, Some(&mut origins))?
                        .ok_or("missing complete module table")?,
                    items
                        .query(archive, &parsed, Some(&mut origins))?
                        .ok_or("missing complete mission item table")?,
                ]
            };
            let counts =
                measurement.verify_tables(FAMILIES.map(Family::path), &tables, &origins)?;
            print_line(&format!(
                "complete mission definition SDK parity: rows={:?} sourceUses={} physicalSources={}",
                counts.rows, counts.source_uses, counts.physical_sources,
            ));
            Ok(())
        },
    )
}

#[test]
#[ignore = "standalone cold query: configure the installed Archive and one exact dataset path"]
fn installed_mission_definition_cold_query() -> TestResult {
    let path = env::var("P4KX_MISSION_DEFINITION_SDK_COLD_DATASET")?;
    let family = FAMILIES
        .into_iter()
        .find(|family| family.path() == path)
        .ok_or("configure one exact mission definition dataset path")?;
    equipment_input::with_archive(|archive| {
        let session =
            CatalogQuerySession::open(archive, CatalogLanguageSelection::AllAvailable, None)?;
        let parsed = session.parsed(None)?;
        let _ = family.query(archive, &parsed, None)?;
        Ok(())
    })
}

#[test]
fn mission_definition_parity_requires_the_present_physical_record() -> TestResult {
    use serde_json::json;

    let row = json!({"recordIndex": {"state": "present", "value": 19}});
    let foreign = json!({"owner": {"ownerRecordIndex": {"state": "present", "value": 19}}});
    let unavailable = json!({"recordIndex": {"state": "unavailable", "value": 19}});
    assert_eq!(
        wire_record_index(&row)?,
        19,
        "the original definition Record"
    );
    assert!(
        wire_record_index(&foreign).is_err(),
        "a component owner is a different physical identity"
    );
    assert!(
        wire_record_index(&unavailable).is_err(),
        "the physical Record index must be present"
    );
    Ok(())
}
