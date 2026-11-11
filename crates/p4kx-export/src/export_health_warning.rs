// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::crafting_warning::CraftingWarning;
use crate::export_diagnostic::{
    ExportDiagnosticCode, ExportDiagnosticSeverity, ExportDiagnosticStage, ExportDiagnosticUnit,
};
use crate::health_diagnostic_observation::{
    DiagnosticObservation as ExportDiagnostic, ExampleObservation,
};
use crate::selection::Kind;

/// Maps the established source-warning vocabulary to its affected typed projections and units.
pub(crate) fn diagnostics(warning: &CraftingWarning) -> impl Iterator<Item = ExportDiagnostic> {
    let (kinds, unit, stage) = affected(warning);
    kinds.iter().map(move |kind| {
        source_warning(
            *kind,
            stage,
            warning.kind(),
            warning.count().unwrap_or(1),
            unit,
            Vec::new(),
        )
    })
}

pub(crate) fn source_warning(
    kind: Kind,
    stage: ExportDiagnosticStage,
    warning_code: &str,
    count: usize,
    count_unit: ExportDiagnosticUnit,
    examples: Vec<ExampleObservation>,
) -> ExportDiagnostic {
    ExportDiagnostic {
        kind: Some(kind),
        stage,
        code: ExportDiagnosticCode::SourceWarning,
        severity: ExportDiagnosticSeverity::Warning,
        source_role: None,
        warning_code: Some(warning_code.to_owned()),
        count,
        count_unit,
        examples,
    }
}

const fn affected(
    warning: &CraftingWarning,
) -> (&'static [Kind], ExportDiagnosticUnit, ExportDiagnosticStage) {
    use ExportDiagnosticStage::{DerivedProjection, Projection, SourceRead};
    use ExportDiagnosticUnit::{ArchiveEntries, Occurrences, Records, Rows, Values};
    use Kind::{
        BlueprintPools, Blueprints, DismantleProcesses, HarvestableLocations,
        HarvestableProviderPresets, LootContainers, MineableEntities, Paints, Shops,
        StarmapObjects, SubHarvestableMultiConfigs,
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
        CraftingWarning::UnknownTemplateValues { .. } => (&[], Values, Projection),
    }
}
