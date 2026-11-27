// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::Kind;
use crate::catalog_number_capture::{FloatToken, NumberOccurrence};
use crate::catalog_v3::loot_definition_catalog::has_authored_arena;
use crate::resource_raw_policy::ResourceNumberContract;

/// Whether this kind covers original numeric data by owned occurrence.
pub(super) const fn uses_owned_number(kind: Kind) -> bool {
    has_authored_arena(kind)
        || matches!(
            kind,
            Kind::Blueprints
                | Kind::Ships
                | Kind::BlueprintPools
                | Kind::DismantleProcesses
                | Kind::Items
                | Kind::ExplosionDefinitions
                | Kind::AmmoDefinitions
                | Kind::InventoryContainers
                | Kind::MedicalItemTierConfigs
                | Kind::Factions
                | Kind::ProceduralPlanetAudio
                | Kind::ResourceNetworkGlobals
                | Kind::CargoLoadingGlobals
                | Kind::ShopCommodityGlobals
                | Kind::FactionIdentities
                | Kind::FactionReputations
                | Kind::ReputationScopes
                | Kind::ReputationStandings
                | Kind::ReputationContexts
                | Kind::ReputationPerks
                | Kind::Infractions
                | Kind::InfractionSets
                | Kind::Jurisdictions
                | Kind::SecurityNetworkManifests
                | Kind::CargoManifests
                | Kind::MissionLocalities
                | Kind::MissionScenarios
                | Kind::MissionModuleDeclarations
                | Kind::MissionItems
                | Kind::LocationClasses
                | Kind::LocationFunctions
                | Kind::LocationSizes
                | Kind::LocationResourceSlots
                | Kind::LocationEntityDeclarations
                | Kind::LocationClassRegistries
                | Kind::LocationFunctionRegistries
                | Kind::LocationSizeRegistries
                | Kind::ContractScopes
                | Kind::NpcArchetypes
                | Kind::NpcClassPools
                | Kind::AutoSpawns
                | Kind::AutoSpawnSets
                | Kind::EncounterIndices
                | Kind::StarmapObjects
                | Kind::StarmapTypes
                | Kind::StarmapPresets
                | Kind::RefiningProcesses
                | Kind::MiningGlobalParams
                | Kind::StarmapAmenities
        )
}

/// Returns the numeric occurrence and wire contract required by the owning kind.
pub(super) const fn expected_owned_number(
    kind: Kind,
    captured: NumberOccurrence,
    ordinal: u64,
    token: FloatToken,
) -> Option<(NumberOccurrence, ResourceNumberContract)> {
    use NumberOccurrence::{
        AuthoredDefinitionRawValue, AuthoredDefinitionValue, BlueprintValue, ItemComponentRawValue,
        ItemComponentValue, LootValue,
    };
    use ResourceNumberContract::{Double, Number, Single};

    Some(match (kind, captured) {
        (kind, _) if has_authored_arena(kind) => (LootValue { ordinal }, Number),
        (Kind::Blueprints, _) => (BlueprintValue { ordinal }, Number),
        (Kind::Items, ItemComponentRawValue { .. }) => (ItemComponentRawValue { ordinal }, Number),
        (Kind::Items, _) => (ItemComponentValue { ordinal }, Single),
        (kind, AuthoredDefinitionRawValue { .. }) if uses_authored_raw_number(kind) => {
            (AuthoredDefinitionRawValue { ordinal }, Number)
        }
        (Kind::StarmapObjects | Kind::StarmapPresets, AuthoredDefinitionValue { .. })
            if matches!(token, FloatToken::Double(_)) =>
        {
            (AuthoredDefinitionValue { ordinal }, Double)
        }
        (
            Kind::ExplosionDefinitions
            | Kind::AmmoDefinitions
            | Kind::InventoryContainers
            | Kind::ResourceNetworkGlobals
            | Kind::CargoLoadingGlobals
            | Kind::MedicalItemTierConfigs
            | Kind::FactionReputations
            | Kind::ReputationStandings
            | Kind::ReputationContexts
            | Kind::ReputationPerks
            | Kind::Infractions
            | Kind::InfractionSets
            | Kind::Jurisdictions
            | Kind::StarmapObjects
            | Kind::StarmapTypes
            | Kind::StarmapPresets
            | Kind::NpcClassPools
            | Kind::AutoSpawns
            | Kind::AutoSpawnSets
            | Kind::FactionIdentities,
            _,
        ) => (AuthoredDefinitionValue { ordinal }, Single),
        _ => return None,
    })
}

const fn uses_authored_raw_number(kind: Kind) -> bool {
    matches!(
        kind,
        Kind::Ships
            | Kind::BlueprintPools
            | Kind::DismantleProcesses
            | Kind::Crafters
            | Kind::AmmoDefinitions
            | Kind::InventoryContainers
            | Kind::MedicalItemTierConfigs
            | Kind::Factions
            | Kind::ProceduralPlanetAudio
            | Kind::ResourceNetworkGlobals
            | Kind::CargoLoadingGlobals
            | Kind::ShopCommodityGlobals
            | Kind::FactionIdentities
            | Kind::FactionReputations
            | Kind::ReputationScopes
            | Kind::ReputationStandings
            | Kind::ReputationContexts
            | Kind::ReputationPerks
            | Kind::Infractions
            | Kind::InfractionSets
            | Kind::Jurisdictions
            | Kind::SecurityNetworkManifests
            | Kind::CargoManifests
            | Kind::MissionLocalities
            | Kind::MissionScenarios
            | Kind::MissionModuleDeclarations
            | Kind::MissionItems
            | Kind::LocationClasses
            | Kind::LocationFunctions
            | Kind::LocationSizes
            | Kind::LocationResourceSlots
            | Kind::LocationEntityDeclarations
            | Kind::LocationClassRegistries
            | Kind::LocationFunctionRegistries
            | Kind::LocationSizeRegistries
            | Kind::ContractScopes
            | Kind::NpcArchetypes
            | Kind::NpcClassPools
            | Kind::AutoSpawns
            | Kind::AutoSpawnSets
            | Kind::EncounterIndices
            | Kind::StarmapObjects
            | Kind::StarmapTypes
            | Kind::StarmapPresets
            | Kind::RefiningProcesses
            | Kind::MiningGlobalParams
            | Kind::StarmapAmenities
    )
}
