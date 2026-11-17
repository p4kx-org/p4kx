// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt;
use std::time::SystemTime;

use p4kx_channels::PublicPatches;
use p4kx_extraction::{CancelToken, Filter};

use crate::CatalogError;

/// The step of a catalog build that a [`CatalogProgress`] reports, in the order the steps run.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
#[non_exhaustive]
pub enum CatalogPhase {
    /// The crafting data is read, the reward sources of the blueprint pools among it.
    Crafting,
    /// Every Record is checked for the kinds of the catalog.
    #[default]
    Records,
}

/// How far the build of a catalog has come.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct CatalogProgress {
    /// The step the counts belong to; they start again at 0 with each step.
    pub phase: CatalogPhase,
    /// Records of the DataForge checked so far in this step.
    pub records_done: usize,
    /// Records of the DataForge.
    pub records_total: usize,
}

/// The settings of one catalog build.
#[derive(Clone, Copy)]
pub struct CatalogOptions<'a> {
    /// Requires complete reads by default; diagnostic partial publication must be explicit.
    pub publication_mode: crate::CatalogPublicationMode,
    /// Selects owning catalog Records by path; the global tag taxonomy remains complete.
    pub filter: Option<&'a Filter>,
    /// The export time the catalog states.
    pub exported_at: SystemTime,
    /// Stops the build once cancelled.
    pub cancel: Option<&'a CancelToken>,
    /// Called from any worker thread while Records are checked; calls may overtake each other.
    pub progress: Option<&'a (dyn Fn(CatalogProgress) + Sync)>,
    /// The public patches the catalog's source states for the Build; `None` states none.
    pub public_patches: Option<&'a PublicPatches>,
}

impl CatalogOptions<'_> {
    /// Returns options for a complete catalog exported at `exported_at`, without cancellation,
    /// progress or public patches.
    #[must_use]
    pub const fn new(exported_at: SystemTime) -> Self {
        Self {
            publication_mode: crate::CatalogPublicationMode::Normal,
            filter: None,
            exported_at,
            cancel: None,
            progress: None,
            public_patches: None,
        }
    }
}

impl fmt::Debug for CatalogOptions<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CatalogOptions")
            .field("publication_mode", &self.publication_mode)
            .field("filter", &self.filter)
            .field("exported_at", &self.exported_at)
            .field("cancel", &self.cancel)
            .field("progress", &self.progress.is_some())
            .field("public_patches", &self.public_patches)
            .finish()
    }
}

/// A stage boundary in the catalog file writer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum CatalogStage {
    /// Reads the original DataForge Entry from the Archive.
    ReadDataForge,
    /// Begins preparing the DataForge, localization and provenance inputs.
    ReadInputs,
    /// Opens and validates the loaded DataForge.
    OpenDataForge,
    /// Reads and decodes the available localization Entries.
    ReadLocalization,
    /// Indexes the decoded localization inputs.
    IndexLocalization,
    /// Binds the original inputs to the catalog's provenance registry.
    InitializeOrigins,
    /// Reads crafting Records and Blueprint reward sources.
    Crafting,
    /// Locates and walks the system containers.
    WorldPlacements,
    /// Inspects the Records and prepares selected rows.
    Records,
    /// Resolves contextual and derived data.
    Projections,
    /// Compiles the publication health and checks the publication policy.
    ExportHealth,
    /// Freezes the source and contribution inventory.
    Provenance,
    /// Measures the JSON size for the Zstd frame header.
    MeasureJson,
    /// Writes the JSON to the chosen output sink.
    WriteJson,
    /// Resolves default ship and item loadouts.
    ResolveLoadouts,
    /// Projects starmap, harvestable and loot locations.
    ProjectLocations,
    /// Selects the source Records for contract and mission projections.
    ProjectMissions,
    /// Reads the dependencies of the selected mission projections.
    ReadMissionDependencies,
    /// Projects template, broker and mission location definitions.
    ProjectMissionDefinitions,
    /// Projects the selected contract templates.
    ProjectContractTemplates,
    /// Projects the selected mission broker entries.
    ProjectMissionBrokerEntries,
    /// Projects the selected mission location templates.
    ProjectMissionLocationTemplates,
    /// Reads contract generators and assembles their output rows.
    ProjectContractGenerators,
    /// Resolves original references between mission scenarios and factions.
    LinkMissionFactions,
    /// Projects shops and the remaining configuration tables.
    ProjectConfigurations,
    /// Projects shop definitions and inventory.
    ProjectShops,
    /// Joins world jump operators to their owning Records.
    ResolveJumpOperators,
    /// Reads independent DataForge configuration tables.
    ReadIndependentConfigurations,
    /// Projects authored particle libraries.
    ProjectParticleLibraries,
    /// Projects weapon animation sources.
    ProjectWeaponAnimation,
    /// Captures final row counts and their source accounting.
    FinalizeProjections,
    /// Assembles the final Blueprint tables.
    ProjectBlueprintTables,
    /// Resolves the inventory-backed cargo capacities of facility definitions.
    ResolveCargoCapacities,
    /// Joins loadout occurrences to facility and flight configurations.
    ProjectFacilityLoadouts,
    /// Reads original interior routes, room containers and model helpers.
    ProjectFacilityInteriors,
    /// Projects resource networks and their topology declarations.
    ProjectResourceTopology,
    /// Projects Starmap Record placements and attaches the world-placement tables.
    ProjectStarmapPlacements,
}

impl CatalogStage {
    /// Returns the stable identifier printed in catalog phase logs.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ReadDataForge => "read-dataforge",
            Self::ReadInputs => "read-inputs",
            Self::OpenDataForge => "open-dataforge",
            Self::ReadLocalization => "read-localization",
            Self::IndexLocalization => "index-localization",
            Self::InitializeOrigins => "initialize-origins",
            Self::Crafting => "crafting",
            Self::WorldPlacements => "world-placements",
            Self::Records => "records",
            Self::Projections => "projections",
            Self::ExportHealth => "export-health",
            Self::Provenance => "provenance",
            Self::MeasureJson => "measure-json",
            Self::WriteJson => "write-json",
            Self::ResolveLoadouts => "resolve-loadouts",
            Self::ProjectLocations => "project-locations",
            Self::ProjectMissions => "project-missions",
            Self::ReadMissionDependencies => "read-mission-dependencies",
            Self::ProjectMissionDefinitions => "project-mission-definitions",
            Self::ProjectContractTemplates => "project-contract-templates",
            Self::ProjectMissionBrokerEntries => "project-mission-broker-entries",
            Self::ProjectMissionLocationTemplates => "project-mission-location-templates",
            Self::ProjectContractGenerators => "project-contract-generators",
            Self::LinkMissionFactions => "link-mission-factions",
            Self::ProjectConfigurations => "project-configurations",
            Self::ProjectShops => "project-shops",
            Self::ResolveJumpOperators => "resolve-jump-operators",
            Self::ReadIndependentConfigurations => "read-independent-configurations",
            Self::ProjectParticleLibraries => "project-particle-libraries",
            Self::ProjectWeaponAnimation => "project-weapon-animation",
            Self::FinalizeProjections => "finalize-projections",
            Self::ProjectBlueprintTables => "project-blueprint-tables",
            Self::ResolveCargoCapacities => "resolve-cargo-capacities",
            Self::ProjectFacilityLoadouts => "project-facility-loadouts",
            Self::ProjectFacilityInteriors => "project-facility-interiors",
            Self::ProjectResourceTopology => "project-resource-topology",
            Self::ProjectStarmapPlacements => "project-starmap-placements",
        }
    }
}

/// Options and observation for one preparation and write.
#[derive(Clone, Copy)]
pub(crate) struct CatalogRun<'run, 'inputs> {
    pub(crate) options: &'run CatalogOptions<'inputs>,
    report: Option<&'run dyn Fn(CatalogStage)>,
    #[cfg(test)]
    retention: Option<&'run dyn Fn(CatalogRetention)>,
}

#[cfg(test)]
#[derive(Debug, Clone, Copy)]
pub(crate) struct CatalogRetention {
    pub(crate) stage: CatalogStage,
    pub(crate) source_uses: usize,
    pub(crate) packed_source_uses: usize,
    pub(crate) field_inspections: usize,
    pub(crate) inspection_capacity: usize,
    pub(crate) inline_inspection_bytes: usize,
}

impl<'run, 'inputs> CatalogRun<'run, 'inputs> {
    pub(crate) const fn new(
        options: &'run CatalogOptions<'inputs>,
        report: Option<&'run dyn Fn(CatalogStage)>,
    ) -> Self {
        Self {
            options,
            report,
            #[cfg(test)]
            retention: None,
        }
    }

    #[cfg(test)]
    pub(crate) const fn with_retention(mut self, report: &'run dyn Fn(CatalogRetention)) -> Self {
        self.retention = Some(report);
        self
    }

    #[cfg(test)]
    pub(crate) fn capture_retention(
        self,
        stage: CatalogStage,
        uses: &p4kx_extraction::SourceUses,
        ledgers: &[&crate::export_ledger::ExportLedger],
    ) -> Result<(), CatalogError> {
        if let Some(report) = self.retention {
            let source_uses = uses.registered_use_count()?;
            let mut field_inspections = 0;
            let mut inspection_capacity = 0;
            for ledger in ledgers {
                field_inspections += ledger.field_inspections.len();
                inspection_capacity += ledger.field_inspections.capacity();
            }
            report(CatalogRetention {
                stage,
                source_uses,
                packed_source_uses: uses.packed_use_count()?,
                field_inspections,
                inspection_capacity,
                inline_inspection_bytes: inspection_capacity
                    * size_of::<crate::export_field_inspection::FieldInspection>(),
            });
        }
        Ok(())
    }

    pub(crate) fn check_cancelled(self) -> Result<(), CatalogError> {
        if self.options.cancel.is_some_and(CancelToken::is_cancelled) {
            return Err(CatalogError::Cancelled);
        }
        Ok(())
    }

    pub(crate) fn begin(self, stage: CatalogStage) {
        if let Some(report) = self.report {
            report(stage);
        }
    }
}
