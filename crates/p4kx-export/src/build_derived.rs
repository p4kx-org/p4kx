// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;
use std::sync::Arc;

use p4kx_assembly::Entities;
use p4kx_extraction::{Archive, CancelToken, DataForge, Guid};

use crate::broker_entry_read::BrokerEntries;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_v3::tag_catalog::{CapturedTags, TagDependencies};
use crate::collected::Collected;
use crate::contract_loadout::EntityKind;
use crate::contract_template_read::ContractTemplates;
use crate::crafter_hosts::link_hosts;
use crate::crafting_warning::CraftingWarning;
use crate::loadout::{Wanted, loadouts};
use crate::missions;
use crate::paint::paints;
use crate::{CatalogError, CatalogOptions, LoadoutSummary, PaintSummary};

/// Reads the contracts and sub contracts into `collected`.
pub(crate) fn add_contracts<'f>(
    forge: &'f DataForge<'f>,
    projection: &CatalogProjection<'_, 'f>,
    (tags, dependencies): (&CapturedTags<'_, 'f>, Option<TagDependencies<'f>>),
    options: &CatalogOptions<'_>,
    collected: &mut Collected<'f>,
) -> Result<(), CatalogError> {
    let texts = projection.legacy_texts();
    let missions = missions::read(
        forge,
        projection,
        (tags, dependencies),
        &collected.starmap_objects,
        options.filter,
        options.cancel,
    )?;
    collected.ledger.merge(missions.ledger);
    collected.add_contracts(missions.contracts, missions.skipped, missions.failures);
    collected.contract_generators = missions.generators;
    collected.mission_property_sets = missions.property_sets.into_rows();
    let ContractTemplates {
        read,
        pools: template_pools,
        mut configurations,
        unknowns,
    } = missions.siblings.contract_templates;
    let BrokerEntries {
        read: broker_read,
        pools: broker_pools,
        definitions: broker_definitions,
        flows: broker_flows,
        properties: broker_properties,
    } = missions.siblings.broker_entries;
    let mut pools = missions.pools;
    pools.merge(template_pools);
    pools.merge(broker_pools);
    collected.add_spawns(missions.siblings.location_templates, pools, texts);
    collected.add_broker_entries(broker_read, broker_definitions);
    configurations.flows.extend(broker_flows);
    configurations.properties.extend(broker_properties);
    collected.add_contract_templates(read, configurations, unknowns)
}

/// Derives the paints of `collected`, warns about unreadable vehicle files and returns the counts.
///
/// # Errors
///
/// Returns [`CatalogError::Cancelled`] once `cancel` is cancelled.
pub(crate) fn add_paints<'f>(
    forge: &'f DataForge<'f>,
    input: &OriginalCatalogInput<'_, 'f>,
    collected: &mut Collected<'f>,
    cancel: Option<&CancelToken>,
) -> Result<PaintSummary, CatalogError> {
    let derived = paints(
        forge,
        input,
        &collected.items,
        &collected.vehicles,
        &collected.ledger,
        cancel,
    )?;
    collected.ledger.merge(derived.ledger);
    collected.add_failures(derived.failures);
    collected.paints = derived.list;
    let count = derived.summary.unreadable_vehicles;
    if count > 0 {
        let details = derived.unreadable_files;
        let warning = CraftingWarning::UnreadableVehicleFiles { count, details };
        collected.warnings.push(warning);
    }
    Ok(derived.summary)
}

/// Resolves the Default Loadout of every ship and of every item that may have ports, stores the
/// loadouts in `collected` and returns the counts.
pub(crate) fn add_loadouts<'f>(
    archive: &Archive,
    entities: &Arc<Entities<'f>>,
    collected: &mut Collected<'f>,
    input: &OriginalCatalogInput<'_, 'f>,
    cancel: Option<&CancelToken>,
) -> Result<LoadoutSummary, CatalogError> {
    let wanted: Vec<Wanted> = collected
        .ships
        .iter()
        .map(|ship| Wanted {
            record_index: ship.source_record_index,
            guid: ship.guid,
            kind: EntityKind::Ship,
        })
        .chain(collected.port_items.iter().copied())
        .collect();
    let known: HashSet<Guid> = collected
        .items
        .iter()
        .map(|item| item.guid.0)
        .chain(collected.ships.iter().map(|ship| ship.guid.0))
        .collect();
    let built = loadouts(
        archive,
        entities,
        &wanted,
        &known,
        crate::loadout::SelectedSources {
            ledger: &collected.ledger,
            alignment: &collected.item_alignment,
        },
        input,
        cancel,
    )?;
    collected.ledger.merge(built.ledger);
    collected.loadouts = built.entries;
    link_hosts(&mut collected.crafters, &collected.loadouts);
    collected.add_failures(built.failures);
    Ok(built.summary)
}
