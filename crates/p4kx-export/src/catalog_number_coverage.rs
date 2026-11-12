// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::Kind;
use crate::catalog_number_capture::NumberOccurrence;
use crate::resource_raw_policy::ResourceNumberContract;

/// Whether this kind covers original numeric data by owned occurrence.
pub(super) const fn uses_owned_number(kind: Kind) -> bool {
    matches!(
        kind,
        Kind::LootPoolFilters
            | Kind::Blueprints
            | Kind::Items
            | Kind::ExplosionDefinitions
            | Kind::AmmoDefinitions
            | Kind::Factions
            | Kind::FactionReputations
            | Kind::ReputationScopes
            | Kind::ReputationStandings
            | Kind::ReputationContexts
            | Kind::ReputationPerks
            | Kind::Infractions
            | Kind::InfractionSets
            | Kind::Jurisdictions
            | Kind::LocationClasses
            | Kind::LocationFunctions
            | Kind::LocationSizes
            | Kind::LocationClassRegistries
            | Kind::LocationFunctionRegistries
            | Kind::LocationSizeRegistries
    )
}

/// Returns the numeric occurrence and wire contract required by the owning kind.
pub(super) const fn expected_owned_number(
    kind: Kind,
    captured: NumberOccurrence,
    ordinal: u64,
) -> Option<(NumberOccurrence, ResourceNumberContract)> {
    use NumberOccurrence::{
        AuthoredDefinitionRawValue, AuthoredDefinitionValue, BlueprintValue, ItemComponentRawValue,
        ItemComponentValue, LootValue,
    };
    use ResourceNumberContract::{Number, Single};

    Some(match (kind, captured) {
        (Kind::LootPoolFilters, _) => (LootValue { ordinal }, Number),
        (Kind::Blueprints, _) => (BlueprintValue { ordinal }, Number),
        (Kind::Items, ItemComponentRawValue { .. }) => (ItemComponentRawValue { ordinal }, Number),
        (Kind::Items, _) => (ItemComponentValue { ordinal }, Single),
        (
            Kind::Factions
            | Kind::FactionReputations
            | Kind::ReputationScopes
            | Kind::ReputationStandings
            | Kind::ReputationContexts
            | Kind::ReputationPerks
            | Kind::Infractions
            | Kind::InfractionSets
            | Kind::Jurisdictions
            | Kind::LocationClasses
            | Kind::LocationFunctions
            | Kind::LocationSizes
            | Kind::LocationClassRegistries
            | Kind::LocationFunctionRegistries
            | Kind::LocationSizeRegistries,
            AuthoredDefinitionRawValue { .. },
        ) => (AuthoredDefinitionRawValue { ordinal }, Number),
        (
            Kind::ExplosionDefinitions
            | Kind::AmmoDefinitions
            | Kind::FactionReputations
            | Kind::ReputationStandings
            | Kind::ReputationContexts
            | Kind::ReputationPerks
            | Kind::Infractions
            | Kind::InfractionSets
            | Kind::Jurisdictions,
            _,
        ) => (AuthoredDefinitionValue { ordinal }, Single),
        _ => return None,
    })
}
