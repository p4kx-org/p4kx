// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    CancelToken, DataForge, Filter, Record, RecordTree, StructValue, TreeBuilder,
};
use rayon::prelude::*;

use crate::blueprint::as_struct;
use crate::contract_missions::{ContractEntry, ContractFaction, PickupPlace};
use crate::contract_offer::OfferedAt;
use crate::contract_starmap::StarmapObject;
use crate::guid_text::CatalogGuid;
use crate::mission_cooldown::ContractCooldown;
use crate::mission_facts::Levels;
use crate::mission_hauling_levels::HaulingLevels;
use crate::mission_index::{Broker, MissionIndex, Template};
use crate::mission_intros::{Grants, record_grants, resolve};
use crate::mission_layers::{Layer, items, one};
use crate::mission_places::{Places, Prerequisites, Where};
use crate::mission_results::{granted_tags, link};
use crate::mission_shared::{Found, Rewards, Shared};
use crate::mission_siblings::{self, Siblings};
use crate::mission_titles::titles;
use crate::record_fields::{boolean, identity, reference, text};
use crate::selection::{Kind, is_leftover};
use crate::spawn_error::SpawnError;
use crate::spawn_layers::{HandlerSpawns, RecordSpawns};
use crate::spawn_pools::SpawnPools;
use crate::texts::Texts;
use crate::{CatalogError, CatalogFailure};

const CONTAINERS: [&str; 4] = [
    "contracts",
    INTRO_CONTRACTS,
    "legacyContracts",
    "PVPBountyContract",
];
const INTRO_CONTRACTS: &str = "introContracts";

/// The contracts of a DataForge and what was left out.
pub(crate) struct Missions<'f> {
    pub(crate) contracts: Vec<ContractEntry<'f>>,
    pub(crate) skipped: usize,
    pub(crate) failures: Vec<CatalogFailure>,
    pub(crate) pools: SpawnPools<'f>,
    pub(crate) siblings: Siblings<'f>,
}

/// A Record that cannot be read: the error of the DataForge or a [`SpawnError`].
type PieceError = Box<dyn std::error::Error + Send + Sync>;

enum Piece<'f> {
    Contracts(Vec<ContractEntry<'f>>, usize, Grants, SpawnPools<'f>),
    Failed(CatalogFailure),
}

/// What an entry is resolved against: the Records it refers to, the texts and the starmap.
struct Context<'x, 'f> {
    index: &'x MissionIndex<'f>,
    texts: &'x Texts<'f>,
    places: &'x Places<'x, 'f>,
}

/// Reads each of `records` in parallel; a Record that cannot be read is a failure.
fn read_pieces<'f>(
    records: &[Record<'f>],
    context: &Context<'_, 'f>,
    cancel: Option<&CancelToken>,
) -> Result<Vec<Piece<'f>>, CatalogError> {
    records
        .par_iter()
        .map_init(TreeBuilder::new, |trees, record| {
            if cancel.is_some_and(CancelToken::is_cancelled) {
                return Err(CatalogError::Cancelled);
            }
            Ok(piece(trees, record, context).unwrap_or_else(|source| {
                Some(Piece::Failed(CatalogFailure::of(record, source.as_ref())))
            }))
        })
        .filter_map(Result::transpose)
        .collect()
}

/// Reads the contracts of the `ContractGenerator` Records `filter` matches: for each contract an
/// entry, and one for each of its sub contracts, ordered by the text of the GUIDs. The pickup
/// places are the starmap objects `starmap` lists.
///
/// A contract or sub contract whose debug name or Record is a development leftover, one without an
/// `id`, and an entry whose `id` pair an earlier entry already has, is left out and counted.
///
/// # Errors
///
/// Returns [`CatalogError::Cancelled`] once `cancel` is cancelled.
pub(crate) fn read<'f>(
    forge: &'f DataForge<'f>,
    texts: &Texts<'f>,
    starmap: &[StarmapObject<'f>],
    filter: Option<&Filter>,
    cancel: Option<&CancelToken>,
) -> Result<Missions<'f>, CatalogError> {
    let rule = Kind::Contracts.records();
    let records: Vec<Record<'f>> = forge
        .records()
        .filter(|record| {
            record
                .record_path()
                .is_ok_and(|path| rule.matches_path(path) && filter.is_none_or(|f| f.matches(path)))
        })
        .collect();
    let mut index = MissionIndex::read(forge, cancel)?;
    let (pools, failures) = index.take_template_results();
    let mut missions = Missions {
        contracts: Vec::new(),
        skipped: 0,
        failures,
        pools,
        siblings: mission_siblings::read(forge, (texts, &index), filter, cancel)?,
    };
    if records.is_empty() {
        return Ok(missions);
    }
    let places = Places::new(starmap);
    let context = Context {
        index: &index,
        texts,
        places: &places,
    };
    let mut grants = Grants::new();
    for piece in read_pieces(&records, &context, cancel)? {
        match piece {
            Piece::Contracts(contracts, skipped, granted, found) => {
                missions.pools.merge(found);
                missions.contracts.extend(contracts);
                missions.skipped = missions.skipped.saturating_add(skipped);
                for (tag, intros) in granted {
                    grants.entry(tag).or_default().extend(intros);
                }
            }
            Piece::Failed(failure) => missions.failures.push(failure),
        }
    }
    missions.order();
    resolve(&mut missions.contracts, &mut grants);
    Ok(missions)
}

impl Missions<'_> {
    /// Orders the entries by the text of their GUIDs and drops an entry whose `guid` and
    /// `subContractGuid` an earlier one has, counting it as skipped.
    fn order(&mut self) {
        self.contracts.sort_by_cached_key(|entry| {
            (
                entry.guid.text(),
                entry.sub_contract_guid.map(CatalogGuid::text),
                entry.record_guid.text(),
            )
        });
        let listed = self.contracts.len();
        self.contracts.dedup_by(|later, earlier| {
            later.guid.0 == earlier.guid.0 && later.sub_contract_guid == earlier.sub_contract_guid
        });
        self.skipped = self
            .skipped
            .saturating_add(listed.saturating_sub(self.contracts.len()));
    }
}

fn piece<'f>(
    trees: &mut TreeBuilder<'f>,
    record: &Record<'f>,
    context: &Context<'_, 'f>,
) -> Result<Option<Piece<'f>>, PieceError> {
    let rule = Kind::Contracts.records();
    let path = record.record_path()?;
    if record.type_name()? != rule.type_name {
        return Ok(None);
    }
    let tree = trees.build(record)?;
    let mut contracts = Vec::new();
    let mut skipped = 0_usize;
    let mut grants = Grants::new();
    let spawns = RecordSpawns::new(context.index.tags(), tree.root());
    for handler in items(Some(tree.root()), "generators")
        .iter()
        .filter_map(as_struct)
    {
        let handler_layer = Layer::read(one(Some(handler), "contractParams"));
        let handler_where = Where::read(one(Some(handler), "defaultAvailability"), "prerequisites");
        let handler_spawns = spawns.handler(handler)?;
        for container in CONTAINERS {
            for contract in items(Some(handler), container).iter().filter_map(as_struct) {
                let name = text(Some(contract), "debugName");
                let Some(id) = identity(contract) else {
                    skipped = skipped.saturating_add(1);
                    continue;
                };
                if is_leftover(path, name.unwrap_or_default()) {
                    skipped = skipped.saturating_add(1);
                    continue;
                }
                if container == INTRO_CONTRACTS {
                    record_grants(&mut grants, CatalogGuid(id), &granted_tags(contract));
                }
                let parent = Parent {
                    tree: &tree,
                    handler,
                    handler_layer: &handler_layer,
                    handler_where: &handler_where,
                    spawns: &handler_spawns,
                    contract,
                    id: CatalogGuid(id),
                    name,
                };
                skipped = skipped.saturating_add(parent.entries(context, &mut contracts)?);
            }
        }
    }
    Ok(Some(Piece::Contracts(
        contracts,
        skipped,
        grants,
        spawns.into_pools(),
    )))
}

struct Parent<'t, 'f> {
    tree: &'t RecordTree<'f>,
    handler: &'t StructValue<'f>,
    handler_layer: &'t Layer<'f>,
    handler_where: &'t Where,
    spawns: &'t HandlerSpawns<'t, 't, 'f>,
    contract: &'t StructValue<'f>,
    id: CatalogGuid,
    name: Option<&'f str>,
}

impl<'f> Parent<'_, 'f> {
    /// Appends the entries of the contract and its sub contracts and returns how many sub
    /// contracts were left out.
    fn entries(
        &self,
        context: &Context<'_, 'f>,
        out: &mut Vec<ContractEntry<'f>>,
    ) -> Result<usize, SpawnError> {
        let index = context.index;
        let contract_where = Where::read(Some(self.contract), "additionalPrerequisites");
        let template = reference(Some(self.contract), "template").map(|guid| guid.0);
        let template_data = template.and_then(|guid| index.template(guid));
        let contract_layer = Layer::read(one(Some(self.contract), "paramOverrides"));
        let broker = reference(Some(self.contract), "missionBrokerEntry")
            .and_then(|guid| index.broker(guid.0));
        let shared = Shared::read(self.handler, self.contract);
        let spawn_levels = self.spawns.contract(
            self.contract,
            template_data.and_then(|data| data.spawns.as_ref()),
        )?;
        let make = |sub: Option<&StructValue<'f>>| -> Result<ContractEntry<'f>, SpawnError> {
            let spawns = spawn_levels.resolve(sub)?;
            let sub_layer =
                sub.map(|sub| Layer::read(one(Some(sub), "paramOverrides").or(Some(sub))));
            let levels = Levels {
                sub: sub_layer.as_ref(),
                contract: &contract_layer,
                handler: self.handler_layer,
            };
            let facts = shared.facts.facts(sub, &levels, template_data);
            let (pickup, offered_at) = self.places(sub, &contract_where, broker, context);
            Ok(self.entry(
                sub,
                &levels,
                template,
                (template_data, broker),
                (
                    &shared,
                    Found {
                        pickup,
                        offered_at,
                        facts,
                        spawns,
                    },
                ),
                context,
            ))
        };
        out.push(make(None)?);
        let mut skipped = 0_usize;
        for sub in items(Some(self.contract), "subContracts")
            .iter()
            .filter_map(as_struct)
        {
            let name = text(Some(sub), "debugName").unwrap_or_default();
            if identity(sub).is_none() || is_leftover(self.tree.record_path(), name) {
                skipped = skipped.saturating_add(1);
            } else {
                out.push(make(Some(sub))?);
            }
        }
        Ok(skipped)
    }

    /// Returns the pickup places and where the contract or its sub contract `sub` is offered.
    fn places(
        &self,
        sub: Option<&StructValue<'f>>,
        contract_where: &Where,
        broker: Option<&Broker<'f>>,
        context: &Context<'_, 'f>,
    ) -> (Vec<PickupPlace<'f>>, OfferedAt<'f>) {
        let own = sub.map(|sub| Where::read(Some(sub), "additionalPrerequisites"));
        context.places.locate(
            context.index,
            &Prerequisites {
                own: own.as_ref(),
                inherited: [contract_where, self.handler_where],
            },
            broker.and_then(|broker| broker.location),
        )
    }

    fn entry(
        &self,
        sub: Option<&StructValue<'f>>,
        levels: &Levels<'_, 'f>,
        template: Option<p4kx_extraction::Guid>,
        (template_data, broker): (Option<&Template<'f>>, Option<&Broker<'f>>),
        (shared, found): (&Shared<'f>, Found<'f>),
        context: &Context<'_, 'f>,
    ) -> ContractEntry<'f> {
        let Found {
            pickup,
            offered_at,
            facts,
            spawns,
        } = found;
        let (index, texts) = (context.index, context.texts);
        let faction = self.faction(context);
        let rewards = Rewards::read(
            (self.handler, self.contract, sub),
            faction.as_ref(),
            (index, texts),
        );
        let (title, description) = titles(levels, (template_data, broker), (index, texts));
        let mission_type = levels
            .iter()
            .find_map(|level| level.mission_type)
            .or_else(|| template_data.and_then(|data| data.mission_type))
            .or_else(|| broker.and_then(|broker| broker.mission_type));
        ContractEntry {
            guid: self.id,
            sub_contract_guid: sub.and_then(identity).map(CatalogGuid),
            record_guid: CatalogGuid(self.tree.guid()),
            record_path: self.tree.record_path(),
            debug_name: self.name,
            handler: text(Some(self.handler), "debugName"),
            template_guid: template.map(CatalogGuid),
            broker_entry_guid: reference(Some(self.contract), "missionBrokerEntry"),
            not_for_release: boolean(Some(self.contract), "notForRelease"),
            work_in_progress: boolean(Some(self.contract), "workInProgress"),
            title,
            description,
            mission_type_guid: mission_type.map(CatalogGuid),
            mission_type: texts.localised(
                mission_type
                    .and_then(|guid| index.mission_type(guid))
                    .and_then(|kind| kind.name_key),
            ),
            contractor: texts.localised(levels.iter().find_map(|level| level.contractor)),
            faction,
            pickup_places: pickup,
            offered_at,
            giver_faction: rewards.giver_faction,
            blueprint_rewards: rewards.blueprints,
            reputation_gains: rewards.gains,
            min_standing_guid: shared.standing.min,
            max_standing_guid: shared.standing.max,
            standing_requirements: shared.standing.requirements_of(sub),
            cooldown: ContractCooldown::resolve(&shared.cooldown, levels),
            facts,
            ship_spawns: spawns.ship_spawns,
            npc_spawns: spawns.npc_spawns,
            entity_spawns: spawns.entity_spawns,
            location_queries: spawns.location_queries,
            location_prerequisites: spawns.location_prerequisites,
            hauling_orders: HaulingLevels::of(levels, template_data, broker).orders(),
            fixed_rewards: shared.fixed_rewards.clone(),
        }
    }

    /// Returns the `factionReputation` of the handler with its display name.
    fn faction(&self, context: &Context<'_, 'f>) -> Option<ContractFaction<'f>> {
        link(self.handler, "factionReputation").map(|(guid, name)| ContractFaction {
            guid,
            name,
            display_name: context.texts.localised(context.index.faction_name(guid.0)),
        })
    }
}
