// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{
    CatalogPublicationMode, ExportDiagnosticCode, ExportDiagnosticSeverity, ExportDiagnosticStage,
    ExportDiagnosticUnit, ExportEntryCounts, ExportHealthStatus, ExportSourceCounts,
    ExportSourceMode, ExportSourceRole, ExportStageStatus, Kind,
};
use serde_json::{Value, json};

/// Marks authored schema data as diagnostic without claiming any decoder or source inventory ran.
pub(super) fn fixture(catalog: &Value) -> Value {
    let mut kinds = Vec::new();
    let mut written_rows = 0;
    for kind in Kind::ALL {
        let count = row_count(kind, catalog);
        written_rows += count;
        kinds.push(kind_health(kind, count));
    }
    json!({
        "metadata": { "texts": [] },
        "failures": [],
        "status": ExportHealthStatus::Incomplete,
        "read": ExportStageStatus::Incomplete,
        "projection": ExportStageStatus::Incomplete,
        "publicationMode": CatalogPublicationMode::DiagnosticPartial,
        "uniqueSelectedSourceRecords": 0,
        "selectedKindRecords": 0,
        "writtenRows": written_rows,
        "kinds": kinds,
        "failedRecords": [],
        "failedEntries": [],
        "unresolvedReferences": [],
        "diagnostics": [{
            "kind": null,
            "stage": ExportDiagnosticStage::Selection,
            "code": ExportDiagnosticCode::SelectionFailure,
            "severity": ExportDiagnosticSeverity::Error,
            "sourceRole": ExportSourceRole::SelectionCandidate,
            "warningCode": null,
            "count": 1,
            "countUnit": ExportDiagnosticUnit::Occurrences,
            "examples": [],
        }],
    })
}

fn kind_health(kind: Kind, written_rows: usize) -> Value {
    json!({
        "kind": kind,
        "sourceMode": mode(kind),
        "sourceInventoryComplete": false,
        "fieldInventoryComplete": false,
        "read": ExportStageStatus::Incomplete,
        "projection": ExportStageStatus::Incomplete,
        "sourceRecords": ExportSourceCounts::default(),
        "sourceEntries": ExportEntryCounts::default(),
        "writtenRows": written_rows,
        "intentionallySkippedRows": 0,
        "failedDerivedProjections": 0,
        "failedRecords": [],
        "failedEntries": [],
        "unresolvedReferenceCount": 0,
    })
}

fn row_count(kind: Kind, catalog: &Value) -> usize {
    match kind {
        Kind::Blueprints => catalog.pointer("/definitions/blueprints/rows"),
        Kind::ShopInventories => catalog.pointer("/definitions/shopInventories/rows"),
        Kind::ExplosionDefinitions => catalog.pointer("/definitions/explosionDefinitions/rows"),
        Kind::InventoryContainers => catalog.pointer("/definitions/inventoryContainers/rows"),
        Kind::Factions => catalog.pointer("/definitions/factions/rows"),
        Kind::FactionReputations => catalog.pointer("/definitions/factionReputations/rows"),
        Kind::ReputationPerks => catalog.pointer("/definitions/reputationPerks/rows"),
        Kind::ReputationTriggerRecords => {
            catalog.pointer("/definitions/reputationTriggerRecords/rows")
        }
        Kind::ReputationRewardAmounts => {
            catalog.pointer("/definitions/reputationRewardAmounts/rows")
        }
        Kind::AmmoDefinitions => catalog.pointer("/definitions/ammoDefinitions/rows"),
        Kind::Infractions => catalog.pointer("/definitions/infractions/rows"),
        Kind::InfractionSets => catalog.pointer("/definitions/infractionSets/rows"),
        Kind::Jurisdictions => catalog.pointer("/definitions/jurisdictions/rows"),
        Kind::LocationClasses => catalog.pointer("/definitions/locationClasses/rows"),
        Kind::LocationFunctions => catalog.pointer("/definitions/locationFunctions/rows"),
        Kind::LocationSizes => catalog.pointer("/definitions/locationSizes/rows"),
        Kind::LocationClassRegistries => {
            catalog.pointer("/definitions/locationRegistries/classes/rows")
        }
        Kind::LocationFunctionRegistries => {
            catalog.pointer("/definitions/locationRegistries/functions/rows")
        }
        Kind::LocationSizeRegistries => {
            catalog.pointer("/definitions/locationRegistries/sizes/rows")
        }
        Kind::ContractTemplates => catalog.pointer("/definitions/contractTemplates/rows"),
        Kind::MissionBrokerEntries => catalog.pointer("/definitions/missionBrokerEntries/rows"),
        Kind::BlueprintCategories => catalog.pointer("/definitions/blueprintCategories/rows"),
        Kind::GameplayProperties => catalog.pointer("/definitions/gameplayProperties/rows"),
        Kind::LootPoolFilters => catalog.pointer("/definitions/lootPoolFilters/rows"),
        Kind::BlueprintCategoryLists => {
            catalog.pointer("/declarations/blueprintCategoryLists/rows")
        }
        Kind::Manufacturers => catalog.pointer("/definitions/manufacturers/rows"),
        Kind::Tags => catalog.pointer("/definitions/tags/rows"),
        Kind::Commodities => catalog.pointer("/definitions/commodities/rows"),
        Kind::ResourceTypes => catalog.pointer("/definitions/resourceTypes/rows"),
        _ => catalog.get(kind.name()),
    }
    .and_then(Value::as_array)
    .map_or(0, Vec::len)
}

const fn mode(kind: Kind) -> ExportSourceMode {
    match kind {
        Kind::HarvestableLocations | Kind::LootContainers | Kind::Shops | Kind::ShopInventories => {
            ExportSourceMode::ArchiveEntries
        }
        Kind::Paints
        | Kind::Loadouts
        | Kind::ContractShipSpawns
        | Kind::ContractNpcSpawns
        | Kind::ContractEntitySpawns
        | Kind::ContractLocationQueries
        | Kind::MissionPropertySets => ExportSourceMode::RecordInputs,
        _ => ExportSourceMode::Records,
    }
}
