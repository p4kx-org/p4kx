// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{
    CatalogPublicationMode, ExportDiagnostic, ExportDiagnosticCode, ExportDiagnosticSeverity,
    ExportDiagnosticStage, ExportDiagnosticUnit, ExportEntryCounts, ExportHealth,
    ExportHealthStatus, ExportSourceCounts, ExportSourceMode, ExportSourceRole, ExportStageStatus,
    Kind, KindExportHealth,
};
use serde_json::{Value, json};

/// Marks authored schema data as diagnostic without claiming any decoder or source inventory ran.
pub(super) fn fixture(catalog: &Value) -> Value {
    let kinds: Vec<_> = Kind::ALL
        .into_iter()
        .map(|kind| kind_health(kind, catalog))
        .collect();
    json!(ExportHealth {
        status: ExportHealthStatus::Incomplete,
        read: ExportStageStatus::Incomplete,
        projection: ExportStageStatus::Incomplete,
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        unique_selected_source_records: 0,
        selected_kind_records: 0,
        written_rows: kinds.iter().map(|kind| kind.written_rows).sum(),
        kinds,
        failed_records: Vec::new(),
        failed_entries: Vec::new(),
        unresolved_references: Vec::new(),
        diagnostics: vec![ExportDiagnostic {
            kind: None,
            stage: ExportDiagnosticStage::Selection,
            code: ExportDiagnosticCode::SelectionFailure,
            severity: ExportDiagnosticSeverity::Error,
            source_role: Some(ExportSourceRole::SelectionCandidate),
            warning_code: None,
            count: 1,
            count_unit: ExportDiagnosticUnit::Occurrences,
            examples: Vec::new(),
        }],
    })
}

fn kind_health(kind: Kind, catalog: &Value) -> KindExportHealth {
    KindExportHealth {
        kind,
        source_mode: mode(kind),
        source_inventory_complete: false,
        field_inventory_complete: false,
        read: ExportStageStatus::Incomplete,
        projection: ExportStageStatus::Incomplete,
        source_records: ExportSourceCounts::default(),
        source_entries: ExportEntryCounts::default(),
        written_rows: match kind {
            Kind::Manufacturers => catalog.pointer("/definitions/manufacturers/rows"),
            Kind::Commodities => catalog.pointer("/definitions/commodities/rows"),
            Kind::ResourceTypes => catalog.pointer("/definitions/resourceTypes/rows"),
            _ => catalog.get(kind.name()),
        }
        .and_then(Value::as_array)
        .map_or(0, Vec::len),
        intentionally_skipped_rows: 0,
        failed_derived_projections: 0,
        failed_records: Vec::new(),
        failed_entries: Vec::new(),
        unresolved_reference_count: 0,
    }
}

const fn mode(kind: Kind) -> ExportSourceMode {
    match kind {
        Kind::HarvestableLocations | Kind::LootContainers | Kind::Shops => {
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
