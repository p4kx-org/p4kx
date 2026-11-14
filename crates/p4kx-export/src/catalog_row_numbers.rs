// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::catalog_number_capture::CapturedNumber;
use crate::catalog_number_health::EmittedNumber;
use crate::catalog_v3::catalog_resource_network_read::resource_network_global;
use crate::catalog_v3::medical::medical_tier_config;
use crate::catalog_v3::procedural_planet_audio;
use crate::collected::Collected;
use crate::selection::Kind;

pub(super) fn numbers(collected: &Collected<'_>) -> Vec<EmittedNumber> {
    let mut numbers = Vec::new();
    for (kind, values) in definition_number_groups(collected) {
        numbers.extend(
            values
                .iter()
                .cloned()
                .map(|value| EmittedNumber { kind, value }),
        );
    }
    numbers.extend(
        collected
            .item_components
            .numbers
            .iter()
            .cloned()
            .map(|value| EmittedNumber {
                kind: Kind::Items,
                value,
            }),
    );
    numbers
}

fn physical_definition_number_groups<'rows>(
    collected: &'rows Collected<'_>,
) -> impl Iterator<Item = (Kind, &'rows [CapturedNumber])> {
    collected
        .inventory_containers
        .iter()
        .map(|row| (Kind::InventoryContainers, row.numbers.as_slice()))
        .chain(
            medical_tier_config::sources(&collected.medical_item_tier_configs)
                .map(|(kind, _, _, numbers)| (kind, numbers)),
        )
        .chain(
            collected
                .explosion_definitions
                .iter()
                .map(|row| (Kind::ExplosionDefinitions, row.numbers.as_slice())),
        )
        .chain(
            collected
                .ammo_definitions
                .iter()
                .map(|row| (Kind::AmmoDefinitions, row.numbers.as_slice())),
        )
        .chain(
            procedural_planet_audio::sources(&collected.procedural_planet_audio)
                .map(|(kind, _, _, numbers)| (kind, numbers)),
        )
        .chain(
            resource_network_global::sources(&collected.resource_network_globals)
                .map(|(kind, _, _, numbers)| (kind, numbers)),
        )
}

fn definition_number_groups<'rows>(
    collected: &'rows Collected<'_>,
) -> impl Iterator<Item = (Kind, &'rows [CapturedNumber])> {
    collected
        .resource_types
        .iter()
        .map(|row| (Kind::ResourceTypes, row.numbers.as_slice()))
        .chain(
            collected
                .factions
                .iter()
                .map(|row| (Kind::Factions, row.numbers.as_slice())),
        )
        .chain(collected.mission_kinds.reputation_number_groups())
        .chain(collected.law.number_groups())
        .chain(collected.location_vocabulary.number_groups())
        .chain(collected.npc_spawn_definitions.number_groups())
        .chain(
            crate::catalog_v3::faction_identity_definition::sources(&collected.faction_identities)
                .map(|(kind, _, _, numbers)| (kind, numbers)),
        )
        .chain(
            collected
                .mission_kinds
                .definition_sources()
                .map(|(kind, _, _, numbers)| (kind, numbers)),
        )
        .chain(collected.starmap_definitions.number_groups())
        .chain(physical_definition_number_groups(collected))
        .chain(
            collected
                .blueprints
                .iter()
                .map(|row| (Kind::Blueprints, row.definition.numbers.as_slice())),
        )
        .chain(
            collected
                .gameplay_properties
                .iter()
                .map(|row| (Kind::GameplayProperties, row.numbers.as_slice())),
        )
        .chain(
            collected
                .quality
                .distributions
                .iter()
                .map(|row| (Kind::QualityDistributions, row.numbers.as_slice())),
        )
        .chain(
            collected
                .quality
                .quantizations
                .iter()
                .map(|row| (Kind::QualityQuantizations, row.numbers.as_slice())),
        )
        .chain(
            collected
                .loot_kinds
                .definitions
                .sources()
                .map(|(kind, _, _, numbers)| (kind, numbers)),
        )
}
