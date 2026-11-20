// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Filter, Guid};

use crate::catalog_row::{CatalogRow, CatalogRows};
use crate::catalog_v3::catalog_resource_network_read::resource_network_global;
use crate::catalog_v3::medical::medical_tier_config;
use crate::catalog_v3::procedural_planet_audio;
use crate::collected::Collected;
use crate::selection::Kind;

#[path = "mission_definition_output.rs"]
pub(crate) mod mission_definitions;
#[path = "catalog_row_numbers.rs"]
mod row_numbers;
#[path = "catalog_row_spool.rs"]
pub(crate) mod spool;
#[path = "catalog_staged_table.rs"]
pub(crate) mod staged;

/// Captures the actual final row selection independently of successful source projections.
pub(crate) fn snapshot(collected: &mut Collected<'_>, filter: Option<&Filter>) -> CatalogRows {
    let mut emitted = Vec::new();
    append_definition_rows(&mut emitted, collected);
    append_missions(&mut emitted, collected);
    let mut template_numbers = Vec::new();
    let definitions = std::mem::take(&mut collected.contract_templates.proof)
        .into_iter()
        .chain(std::mem::take(&mut collected.broker_definitions.proof));
    for receipt in definitions {
        emitted.push(receipt.row);
        template_numbers.push(receipt.numbers);
    }
    append_gameplay_definitions(&mut emitted, collected);
    emitted.extend(collected.loot_kinds.definitions.emitted());
    CatalogRows {
        emitted,
        filter: filter.cloned(),
        numbers: row_numbers::numbers(collected),
        configuration_numbers: collected.take_configuration_numbers(),
        template_numbers,
        recovery_numbers: Vec::new(),
    }
}

fn append_definition_rows(emitted: &mut Vec<CatalogRow>, collected: &Collected<'_>) {
    append(
        emitted,
        Kind::Manufacturers,
        collected
            .manufacturers
            .iter()
            .map(|row| (row.guid.0, row.record_path)),
    );
    append(
        emitted,
        Kind::Tags,
        collected
            .tags
            .definitions
            .iter()
            .map(|row| (row.source.physical.guid, row.source.physical.record_path)),
    );
    append(
        emitted,
        Kind::Factions,
        collected
            .factions
            .iter()
            .map(|row| (row.physical_guid, row.physical_path)),
    );
    emitted.extend(collected.ships.iter().map(|row| CatalogRow {
        kind: Kind::Ships,
        guid: row.identity.guid.0,
        record_path: row.identity.record_path.to_owned(),
        record_index: Some(row.source_record_index),
    }));
    append_resources(emitted, collected);
    append_explosions_and_ammo(emitted, collected);
    append(
        emitted,
        Kind::Blueprints,
        collected
            .blueprints
            .iter()
            .map(|row| (row.definition.physical_guid.0, row.definition.physical_path)),
    );
    emitted.extend(collected.blueprint_pools.iter().map(|row| CatalogRow {
        kind: Kind::BlueprintPools,
        guid: row.physical_guid,
        record_path: row.physical_path.to_owned(),
        record_index: Some(row.physical_index),
    }));
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
    emitted.extend(crate::catalog_v3::cargo_manifest::catalog_rows(&collected.mission_kinds.manifests));
    emitted.extend(
        collected
            .law
            .sources()
            .chain(collected.location_vocabulary.sources())
            .chain(collected.npc_spawn_definitions.sources())
            .chain(crate::catalog_v3::faction_identity_definition::sources(
                &collected.faction_identities,
            ))
            .chain(collected.mission_kinds.definition_sources())
            .chain(collected.starmap_definitions.sources())
            .chain(procedural_planet_audio::sources(
                &collected.procedural_planet_audio,
            ))
            .chain(resource_network_global::sources(
                &collected.resource_network_globals,
            ))
            .chain(collected.cargo_globals.sources())
            .chain(crate::catalog_v3::mining_process_catalog::sources(
                &collected.refining_processes,
                &collected.mining_global_params,
            ))
            .map(|(kind, guid, path, _)| CatalogRow {
                kind,
                guid,
                record_path: path.to_owned(),
                record_index: None,
            }),
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
    append(
        emitted,
        Kind::MedicalItemTierConfigs,
        medical_tier_config::sources(&collected.medical_item_tier_configs)
            .map(|(_, guid, path, _)| (guid, path)),
    );
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
        record_index: None,
    }));
}
