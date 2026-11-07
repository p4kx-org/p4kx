// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{RecordTree, StructValue};

use crate::blueprint::as_struct;
use crate::contract_missions::{ContractEntry, ContractFaction, PickupPlace};
use crate::contract_offer::OfferedAt;
use crate::guid_text::CatalogGuid;
use crate::mission_cooldown::ContractCooldown;
use crate::mission_facts::Levels;
use crate::mission_hauling_levels::HaulingLevels;
use crate::mission_index::{Broker, MissionIndex, Template};
use crate::mission_layers::{Layer, items, one};
use crate::mission_places::{Places, Prerequisites, Where};
use crate::mission_property_set_registry::MissionPropertySetRegistry;
use crate::mission_results::link;
use crate::mission_shared::{Found, Rewards, Shared};
use crate::mission_titles::titles;
use crate::record_fields::{boolean, identity, reference, text};
use crate::record_read::ReadError;
use crate::selection::is_leftover;
use crate::spawn_layers::HandlerSpawns;
use crate::template_issues::Unknowns;
use crate::texts::Texts;

/// Shared source context for deriving contract rows.
pub(crate) struct MissionEntryContext<'x, 'f> {
    pub(crate) index: &'x MissionIndex<'f>,
    pub(crate) texts: &'x Texts<'f>,
    pub(crate) places: &'x Places<'x, 'f>,
    pub(crate) unknowns: &'x Unknowns,
}

/// The original authored source of one contract and its sub-contract rows.
pub(crate) struct MissionEntrySource<'t, 'f> {
    pub(crate) tree: &'t RecordTree<'f>,
    pub(crate) handler: &'t StructValue<'f>,
    pub(crate) handler_layer: &'t Layer<'f>,
    pub(crate) handler_where: &'t Where,
    pub(crate) spawns: &'t HandlerSpawns<'t, 't, 'f>,
    pub(crate) contract: &'t StructValue<'f>,
    pub(crate) id: CatalogGuid,
    pub(crate) name: Option<&'f str>,
    pub(crate) property_sets: &'t MissionPropertySetRegistry<'t, 'f>,
}

impl<'t, 'f> MissionEntrySource<'t, 'f> {
    /// Appends the entries of the contract and its sub contracts and returns how many sub
    /// contracts were left out.
    pub(crate) fn entries(
        &self,
        context: &MissionEntryContext<'_, 'f>,
        out: &mut Vec<ContractEntry<'f>>,
    ) -> Result<usize, ReadError> {
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
        let make = |sub: Option<&'t StructValue<'f>>| -> Result<ContractEntry<'f>, ReadError> {
            let spawns = spawn_levels.resolve(sub)?;
            let property_set_guids = self.property_sets.links(self.handler, self.contract, sub)?;
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
                        property_set_guids,
                    },
                ),
                context,
            ))
        };
        out.push(make(None)?);
        self.sub_entries(out, |sub| make(Some(sub)))
    }

    fn sub_entries(
        &self,
        out: &mut Vec<ContractEntry<'f>>,
        make: impl Fn(&'t StructValue<'f>) -> Result<ContractEntry<'f>, ReadError>,
    ) -> Result<usize, ReadError> {
        let mut skipped = 0_usize;
        for sub in items(Some(self.contract), "subContracts")
            .iter()
            .filter_map(as_struct)
        {
            let name = text(Some(sub), "debugName").unwrap_or_default();
            if identity(sub).is_none() || is_leftover(self.tree.record_path(), name) {
                skipped = skipped.saturating_add(1);
            } else {
                out.push(make(sub)?);
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
        context: &MissionEntryContext<'_, 'f>,
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
        context: &MissionEntryContext<'_, 'f>,
    ) -> ContractEntry<'f> {
        let Found {
            pickup,
            offered_at,
            facts,
            spawns,
            property_set_guids,
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
            property_set_guids,
            location_prerequisites: spawns.location_prerequisites,
            hauling_orders: HaulingLevels::of(levels, template_data, broker).orders(),
            fixed_rewards: shared.fixed_rewards.clone(),
        }
    }

    /// Returns the `factionReputation` of the handler with its display name.
    fn faction(&self, context: &MissionEntryContext<'_, 'f>) -> Option<ContractFaction<'f>> {
        link(self.handler, "factionReputation").map(|(guid, name)| ContractFaction {
            guid,
            name,
            display_name: context.texts.localised(context.index.faction_name(guid.0)),
        })
    }
}
