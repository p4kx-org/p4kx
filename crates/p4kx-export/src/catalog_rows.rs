// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Filter, Guid};

use crate::catalog_number_capture::CapturedNumber;
use crate::catalog_number_health::EmittedNumber;
use crate::catalog_row::{CatalogRow, CatalogRows};
use crate::collected::Collected;
use crate::selection::Kind;
use crate::template_number_capture::TemplateNumbers;

/// Captures the actual final row selection independently of successful source projections.
pub(crate) fn snapshot(collected: &mut Collected<'_>, filter: Option<&Filter>) -> CatalogRows {
    let mut emitted = Vec::new();
    append(
        &mut emitted,
        Kind::Manufacturers,
        collected
            .manufacturers
            .iter()
            .map(|row| (row.guid.0, row.record_path)),
    );
    append(
        &mut emitted,
        Kind::Tags,
        collected
            .tags
            .definitions
            .iter()
            .map(|row| (row.source.physical.guid, row.source.physical.record_path)),
    );
    append(
        &mut emitted,
        Kind::Factions,
        collected
            .factions
            .iter()
            .map(|row| (row.physical_guid, row.physical_path)),
    );
    append_resources(&mut emitted, collected);
    append_explosions_and_ammo(&mut emitted, collected);
    append(
        &mut emitted,
        Kind::Blueprints,
        collected
            .blueprints
            .iter()
            .map(|row| (row.definition.physical_guid.0, row.definition.physical_path)),
    );
    append_missions(&mut emitted, collected);
    append_gameplay_definitions(&mut emitted, collected);
    append(
        &mut emitted,
        Kind::StarmapObjects,
        collected
            .starmap_objects
            .iter()
            .map(|row| (row.guid.0, row.record_path)),
    );
    CatalogRows {
        emitted,
        filter: filter.cloned(),
        numbers: numbers(collected),
        configuration_numbers: std::mem::take(&mut collected.entity_configurations.numbers),
        template_numbers: template_numbers(collected),
        recovery_numbers: Vec::new(),
    }
}

fn append_explosions_and_ammo(emitted: &mut Vec<CatalogRow>, collected: &Collected<'_>) {
    append(
        emitted,
        Kind::ExplosionDefinitions,
        collected
            .explosion_definitions
            .iter()
            .map(|row| (row.physical_guid, row.physical_path)),
    );
    append(
        emitted,
        Kind::AmmoDefinitions,
        collected
            .ammo_definitions
            .iter()
            .map(|row| (row.physical_guid, row.physical_path)),
    );
}

fn append_gameplay_definitions(emitted: &mut Vec<CatalogRow>, collected: &Collected<'_>) {
    emitted.extend(
        collected
            .law
            .sources()
            .chain(collected.location_vocabulary.sources())
            .map(|(kind, guid, path, _)| CatalogRow {
                kind,
                guid,
                record_path: path.to_owned(),
            }),
    );
    append(
        emitted,
        Kind::LootPoolFilters,
        collected
            .loot_kinds
            .filters
            .iter()
            .map(|row| (row.physical_guid.0, row.physical_path)),
    );
    append(
        emitted,
        Kind::GameplayProperties,
        collected
            .gameplay_properties
            .iter()
            .map(|row| (row.physical_guid.0, row.physical_path)),
    );
    append(
        emitted,
        Kind::InventoryContainers,
        collected
            .inventory_containers
            .iter()
            .map(|row| (row.physical_guid, row.physical_path)),
    );
}

fn template_numbers(collected: &Collected<'_>) -> Vec<TemplateNumbers> {
    collected
        .contract_templates
        .iter()
        .map(|row| row.numbers.clone())
        .chain(
            collected
                .broker_definitions
                .iter()
                .map(|row| row.numbers.clone()),
        )
        .collect()
}

fn append_reputation_definitions(
    emitted: &mut Vec<CatalogRow>,
    missions: &crate::mission_kinds::MissionKinds<'_>,
) {
    append(
        emitted,
        Kind::FactionReputations,
        missions
            .factions
            .iter()
            .map(|row| (row.physical_guid, row.physical_path)),
    );
    append(
        emitted,
        Kind::ReputationContexts,
        missions
            .contexts
            .iter()
            .map(|row| (row.physical_guid, row.physical_path)),
    );
    append(
        emitted,
        Kind::ReputationPerks,
        missions
            .perks
            .iter()
            .map(|row| (row.physical_guid, row.physical_path)),
    );
    append(
        emitted,
        Kind::ReputationScopes,
        missions
            .scopes
            .iter()
            .map(|row| (row.physical_guid, row.physical_path)),
    );
    append(
        emitted,
        Kind::ReputationStandings,
        missions
            .standings
            .iter()
            .map(|row| (row.physical_guid, row.physical_path)),
    );
}

fn append_missions(emitted: &mut Vec<CatalogRow>, collected: &Collected<'_>) {
    append(
        emitted,
        Kind::ReputationTriggerRecords,
        collected
            .mission_kinds
            .trigger_records
            .iter()
            .map(|row| (row.physical_guid, row.physical_path)),
    );
    append(
        emitted,
        Kind::ReputationRewardAmounts,
        collected
            .mission_kinds
            .reward_amounts
            .iter()
            .map(|row| (row.physical_guid, row.physical_path)),
    );
    append_reputation_definitions(emitted, &collected.mission_kinds);
    append(
        emitted,
        Kind::ContractTemplates,
        collected
            .contract_templates
            .iter()
            .map(|row| (row.physical_guid.0, row.physical_path)),
    );
    append(
        emitted,
        Kind::MissionBrokerEntries,
        collected
            .broker_definitions
            .iter()
            .map(|row| (row.physical_guid.0, row.physical_path)),
    );
}

fn append_resources(emitted: &mut Vec<CatalogRow>, collected: &Collected<'_>) {
    append(
        emitted,
        Kind::Commodities,
        collected
            .commodities
            .iter()
            .map(|row| (row.guid.0, row.record_path)),
    );
    append(
        emitted,
        Kind::ResourceTypes,
        collected
            .resource_types
            .iter()
            .map(|row| (row.guid.0, row.record_path)),
    );
    append(
        emitted,
        Kind::ResourceGroups,
        collected
            .resource_groups
            .iter()
            .map(|row| (row.guid.0, row.record_path)),
    );
    append(
        emitted,
        Kind::QualityDistributions,
        collected
            .quality
            .distributions
            .iter()
            .map(|row| (row.owner.guid.0, row.owner.record_path)),
    );
    append(
        emitted,
        Kind::QualityQuantizations,
        collected
            .quality
            .quantizations
            .iter()
            .map(|row| (row.owner.guid.0, row.owner.record_path)),
    );
}

fn append<'source>(
    emitted: &mut Vec<CatalogRow>,
    kind: Kind,
    rows: impl Iterator<Item = (Guid, &'source str)>,
) {
    emitted.extend(rows.map(|(guid, record_path)| CatalogRow {
        kind,
        guid,
        record_path: record_path.to_owned(),
    }));
}

fn numbers(collected: &Collected<'_>) -> Vec<EmittedNumber> {
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
                .filters
                .iter()
                .map(|row| (Kind::LootPoolFilters, row.numbers.as_slice())),
        )
}
