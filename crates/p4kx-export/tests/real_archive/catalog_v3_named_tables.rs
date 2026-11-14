// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::Kind;

pub(super) const fn uses_named_definition_table(kind: Kind) -> bool {
    matches!(
        kind,
        Kind::NpcArchetypes
            | Kind::NpcClassPools
            | Kind::AutoSpawns
            | Kind::AutoSpawnSets
            | Kind::PlanetResources
            | Kind::Creatures
            | Kind::ClimateAssets
            | Kind::GeologyAssets
            | Kind::Ecosystems
            | Kind::GeologySets
            | Kind::GeologyTextureSets
            | Kind::Soils
            | Kind::SoilTextureSets
            | Kind::ShadingClasses
            | Kind::InventoryContainers
            | Kind::MedicalItemTierConfigs
            | Kind::ParticleLibraries
            | Kind::EncounterIndices
            | Kind::MissionItems
            | Kind::ProceduralPlanetAudio
            | Kind::ContractScopes
            | Kind::LootPoolFilters
            | Kind::LootSecondaryChoiceSingleLayers
            | Kind::LootSecondaryChoiceMultiLayers
            | Kind::LootThemes
            | Kind::HierarchicalContextualDataPresets
            | Kind::LootGenerationGlobalParams
            | Kind::SubHarvestableConfigs
            | Kind::SubHarvestableMultiConfigs
    )
}
