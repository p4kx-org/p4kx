// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::crafting_warning::CraftingWarning;
use crate::export_diagnostic::{
    ExportDiagnosticCode, ExportDiagnosticSeverity, ExportDiagnosticStage, ExportDiagnosticUnit,
};
use crate::health_diagnostic_observation::DiagnosticObservation as ExportDiagnostic;
use crate::selection::Kind;

/// Maps the established source-warning vocabulary to its affected typed projections and units.
pub(crate) fn diagnostics(warning: &CraftingWarning) -> Vec<ExportDiagnostic> {
    let (kinds, unit, stage) = affected(warning);
    kinds
        .iter()
        .map(|kind| ExportDiagnostic {
            kind: Some(*kind),
            stage,
            code: ExportDiagnosticCode::SourceWarning,
            severity: ExportDiagnosticSeverity::Warning,
            source_role: None,
            warning_code: Some(warning.kind().to_owned()),
            count: warning.count().unwrap_or(1),
            count_unit: unit,
            examples: Vec::new(),
        })
        .collect()
}

const fn affected(
    warning: &CraftingWarning,
) -> (&'static [Kind], ExportDiagnosticUnit, ExportDiagnosticStage) {
    use ExportDiagnosticStage::{DerivedProjection, Projection, SourceRead};
    use ExportDiagnosticUnit::{ArchiveEntries, Occurrences, Records, Rows, Values};
    use Kind::{
        BlueprintPools, Blueprints, ContractTemplates, DismantleProcesses, HarvestableLocations,
        HarvestableProviderPresets, LootContainers, MineableEntities, MissionBrokerEntries, Paints,
        Shops, StarmapObjects, SubHarvestableMultiConfigs,
    };

    match warning {
        CraftingWarning::UnreadableRewardRecords(_) => {
            (&[BlueprintPools, Blueprints], Records, SourceRead)
        }
        CraftingWarning::UnknownModifierRanges { .. } => (&[Blueprints], Values, Projection),
        CraftingWarning::AmbiguousDismantle(_) => (
            &[Blueprints, DismantleProcesses],
            Records,
            DerivedProjection,
        ),
        CraftingWarning::UnreadableVehicleFiles { .. } => (&[Paints], Records, DerivedProjection),
        CraftingWarning::UnplacedStarmapContainers(_) => {
            (&[StarmapObjects], Occurrences, DerivedProjection)
        }
        CraftingWarning::UnreadableHarvestContainers { .. } => {
            (&[HarvestableLocations], ArchiveEntries, SourceRead)
        }
        CraftingWarning::HarvestLocationsWithoutObject { .. } => {
            (&[HarvestableLocations], Rows, DerivedProjection)
        }
        CraftingWarning::UnreadableLootContainers { .. } => {
            (&[LootContainers], Occurrences, SourceRead)
        }
        CraftingWarning::AmbiguousLootEntityFiles { .. } => {
            (&[LootContainers], ArchiveEntries, SourceRead)
        }
        CraftingWarning::UnreadableHarvestables { .. } => (
            &[HarvestableProviderPresets, SubHarvestableMultiConfigs],
            Records,
            SourceRead,
        ),
        CraftingWarning::UnreadableShopRegistry
        | CraftingWarning::MissingShopRegistry
        | CraftingWarning::UnreadableShopContainers(_)
        | CraftingWarning::UnreadableShopInventories { .. }
        | CraftingWarning::MissingShopInventories(_) => (&[Shops], ArchiveEntries, SourceRead),
        CraftingWarning::NoShopComponents => (&[Shops], Occurrences, DerivedProjection),
        CraftingWarning::UnknownScanSignatureSlot(_) => {
            (&[MineableEntities], Occurrences, Projection)
        }
        CraftingWarning::UnknownTemplateValues { .. } => (
            &[ContractTemplates, MissionBrokerEntries],
            Values,
            Projection,
        ),
    }
}
