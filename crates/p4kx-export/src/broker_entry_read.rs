// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;

use p4kx_extraction::{CancelToken, DataForge, Filter, RecordTree, StructValue};

use crate::CatalogError;
use crate::broker_entry::BrokerEntry;
use crate::broker_identity::{module, names};
use crate::broker_links::{links, tags};
use crate::broker_requirements::{requirements, results, schedule};
use crate::broker_terms::{availability, cooldowns, finish, instances, reward};
use crate::contract_hauling::HaulingOrder;
use crate::contract_template_read::{properties, settings};
use crate::guid_text::CatalogGuid;
use crate::mission_hauling::objective_orders;
use crate::mission_hauling_levels::HaulingLevels;
use crate::mission_index::MissionIndex;
use crate::record_fields::{boolean, class_name, integer};
use crate::record_read::{KindRead, ReadError, read_kind};
use crate::selection::Kind;
use crate::spawn_layers::{Level, SpawnPools};
use crate::spawn_model::SpawnLevel;
use crate::spawn_read::Reader;
use crate::template_extras::{owner, partial_payout};
use crate::template_flow::FlowReader;
use crate::template_issues::Unknowns;
use crate::template_modifier::modifiers;
use crate::template_objective_read::ObjectiveReader;
use crate::texts::Texts;

/// The mission broker entries of a DataForge, what was left out and the ship spawns and location
/// queries their properties state.
pub(crate) struct BrokerEntries<'f> {
    pub(crate) read: KindRead<BrokerEntry<'f>>,
    pub(crate) pools: SpawnPools,
}

/// What an entry is read with.
struct Sources<'r, 'f> {
    forge: &'r DataForge<'f>,
    texts: &'r Texts<'f>,
    index: &'r MissionIndex<'f>,
    unknowns: &'r Unknowns,
}

/// Reads the `MissionBrokerEntry` Records `filter` matches, ordered by GUID; a development leftover
/// is left out and counted. A Record that nests its phases or conditions too deep or holds a
/// struct the catalog does not know is a failure.
///
/// # Errors
///
/// Returns [`CatalogError::Cancelled`] once `cancel` is cancelled.
pub(crate) fn read<'f>(
    forge: &'f DataForge<'f>,
    (texts, index, unknowns): (&Texts<'f>, &MissionIndex<'f>, &Unknowns),
    filter: Option<&Filter>,
    cancel: Option<&CancelToken>,
) -> Result<BrokerEntries<'f>, CatalogError> {
    let sources = Sources {
        forge,
        texts,
        index,
        unknowns,
    };
    let read = read_kind(
        forge,
        Kind::MissionBrokerEntries,
        (filter, cancel),
        |tree| entry(&sources, tree),
        |(entry, _)| entry.guid,
    )?;
    let mut pools = SpawnPools::default();
    let items = read
        .items
        .into_iter()
        .map(|(entry, found)| {
            pools.merge(found);
            entry
        })
        .collect();
    Ok(BrokerEntries {
        read: KindRead {
            items,
            skipped: read.skipped,
            failures: read.failures,
        },
        pools,
    })
}

fn entry<'f>(
    sources: &Sources<'_, 'f>,
    tree: &RecordTree<'f>,
) -> Result<(BrokerEntry<'f>, SpawnPools), ReadError> {
    let Sources {
        forge,
        texts,
        index,
        unknowns,
    } = *sources;
    let root = Some(tree.root());
    let reader = Reader::new(index.tags(), tree.root());
    let pools = RefCell::new(SpawnPools::default());
    let spawns = Level::read(
        SpawnLevel::Broker,
        &reader,
        (root, "properties"),
        (None, ""),
        &mut pools.borrow_mut(),
    )?
    .resolved();
    let flow = FlowReader {
        reader: &reader,
        unknowns,
    };
    let entry = BrokerEntry {
        guid: CatalogGuid(tree.guid()),
        class_name: class_name(tree),
        record_path: tree.record_path(),
        not_for_release: boolean(root, "notForRelease"),
        names: names(texts, root),
        links: links(root),
        module: module(root),
        tags: tags(&reader, root),
        owner: owner(forge, root),
        difficulty: integer(root, "missionDifficulty"),
        pre_show_objectives: boolean(root, "preShowObjectives"),
        show_life_time_in_mobi_glas: boolean(root, "showLifeTimeInMobiGlas"),
        settings: settings(tree.root()),
        availability: availability(root),
        schedule: schedule(unknowns, root),
        cooldowns: cooldowns(root),
        instances: instances(root),
        reward: reward(root),
        buy_in_amount: integer(root, "missionBuyInAmount"),
        refund_buy_in_on_withdraw: boolean(root, "refundBuyInOnWithdraw"),
        finish: finish(texts, unknowns, root),
        standing_requirements: requirements(unknowns, root)?,
        reputation_results: results(index, root),
        mission_properties: properties(root, "properties"),
        ship_spawns: spawns.ship_spawns,
        location_queries: spawns.location_queries,
        hauling_orders: hauling_orders(root),
        objectives: ObjectiveReader::new(&reader, texts, unknowns, &pools).objectives(root)?,
        partial_payout: partial_payout(root),
        modifiers: modifiers(&reader, unknowns, root)?,
        mission_flow: flow.triggers(root)?,
    };
    Ok((entry, pools.into_inner()))
}

/// Returns the orders of the hauling objectives of the entry `root`.
fn hauling_orders<'f>(root: Option<&StructValue<'f>>) -> Vec<HaulingOrder<'f>> {
    let objective = objective_orders(root);
    HaulingLevels {
        properties: Vec::new(),
        template_objective: &[],
        broker_objective: &objective,
    }
    .orders()
}
