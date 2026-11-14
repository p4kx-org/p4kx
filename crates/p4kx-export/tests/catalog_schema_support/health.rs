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
        Kind::LocationClassRegistries => {
            catalog.pointer("/definitions/locationRegistries/classes/rows")
        }
        Kind::LocationFunctionRegistries => {
            catalog.pointer("/definitions/locationRegistries/functions/rows")
        }
        Kind::LocationSizeRegistries => {
            catalog.pointer("/definitions/locationRegistries/sizes/rows")
        }
        Kind::BlueprintCategoryLists => {
            catalog.pointer("/declarations/blueprintCategoryLists/rows")
        }
        _ => catalog
            .get("definitions")
            .and_then(|definitions| definitions.get(kind.name()))
            .and_then(|table| table.get("rows"))
            .or_else(|| catalog.get(kind.name())),
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
