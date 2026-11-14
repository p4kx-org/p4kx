// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, Guid, RecordTree, StructValue};

use crate::CatalogError;
use crate::blueprint::as_struct;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_read::original::OriginalFlowFields;
use crate::contract_missions::{ContractBase, ContractFaction, PickupPlace};
use crate::contract_offer::OfferedAt;
use crate::guid_text::CatalogGuid;
use crate::mission_cooldown::PreparedCooldown;
use crate::mission_facts::{Levels, PreparedFacts};
use crate::mission_generator::{ContractOccurrence, ContractOwner, ContractSelection};
use crate::mission_hauling_levels::HaulingLevels;
use crate::mission_index::{Broker, MissionIndex, Template};
use crate::mission_intros::PendingContract;
use crate::mission_layers::{Layer, items, one};
use crate::mission_places::{Places, Prerequisites, Where};
use crate::mission_property_set_registry::MissionPropertySetRegistry;
use crate::mission_results::link;
use crate::mission_shared::{Found, Rewards, Shared};
use crate::mission_text_inputs::{
    BrokerTexts, LayerKind, LayerTexts, TemplateTexts, TextDependencyLinks,
};
use crate::mission_titles::{self, Dependencies, Inputs, Layers, MissionTitles};
use crate::record_fields::{boolean, identity, reference, text};
use crate::record_read::ReadError;
use crate::selection::is_leftover;
use crate::spawn_layers::HandlerSpawns;
use crate::texts::Texts;

/// Shared source context for deriving contract rows.
pub(crate) struct MissionEntryContext<'x, 'f> {
    pub(crate) index: &'x MissionIndex<'f>,
    pub(crate) texts: &'x Texts<'f>,
    pub(crate) projection: &'x CatalogProjection<'x, 'f>,
    pub(crate) places: &'x Places<'x, 'f>,
    pub(crate) cancel: Option<&'x CancelToken>,
}

/// The original authored source of one contract and its sub-contract rows.
pub(crate) struct MissionEntrySource<'t, 'f> {
    pub(crate) tree: &'t RecordTree<'f>,
    pub(crate) handler: &'t StructValue<'f>,
    pub(crate) handler_layer: &'t Layer<'f>,
    pub(crate) handler_texts: &'t LayerTexts<'f>,
    pub(crate) handler_where: &'t Where,
    pub(crate) spawns: &'t HandlerSpawns<'t, 't, 'f>,
    pub(crate) contract: &'t StructValue<'f>,
    pub(crate) name: Option<&'f str>,
    pub(crate) property_sets: &'t MissionPropertySetRegistry<'t, 'f>,
}

impl<'t, 'f> MissionEntrySource<'t, 'f> {
    /// Appends the entries of the contract and its sub contracts and returns how many sub
    /// contracts were left out.
    pub(crate) fn entries(
        &self,
        context: &MissionEntryContext<'_, 'f>,
        owner: &ContractOwner<'_, '_, '_, 'f>,
        out: &mut Vec<PendingContract<'f>>,
    ) -> Result<usize, ReadError> {
        let contract_where = Where::read(Some(self.contract), "additionalPrerequisites");
        let template = reference(Some(self.contract), "template").map(|guid| guid.0);
        let template_data = template.and_then(|guid| context.index.template(guid));
        let contract_layer = Layer::read(one(Some(self.contract), "paramOverrides"));
        let broker = reference(Some(self.contract), "missionBrokerEntry")
            .and_then(|guid| context.index.broker(guid.0));
        let shared = Shared::read(owner.fields(), self.handler, self.contract)?;
        let spawn_levels = self.spawns.contract(
            self.contract,
            template_data.and_then(|data| data.spawns.as_ref()),
        )?;
        let prepared_titles = PreparedTitles::read(
            self.contract,
            self.handler_texts,
            owner,
            context,
            template_data,
            broker,
        )?;
        let (cooldown, prepared_facts) = self.prepare_facts(owner, context)?;
        let make = |sub: Option<&'t StructValue<'f>>| -> Result<PendingContract<'f>, ReadError> {
            let spawns = spawn_levels.resolve(sub)?;
            let property_set_guids = self.property_sets.links(self.handler, self.contract, sub)?;
            let sub_layer =
                sub.map(|sub| Layer::read(one(Some(sub), "paramOverrides").or(Some(sub))));
            let levels = Levels {
                sub: sub_layer.as_ref(),
                contract: &contract_layer,
                handler: self.handler_layer,
            };
            let facts = shared.facts.facts(prepared_facts.resolve(sub)?);
            let requirements = shared.facts.requirements.row(owner.fields(), sub)?;
            let (pickup, offered_at) = self.places(sub, &contract_where, broker, context);
            let selection = owner.selection(sub)?;
            let titles = prepared_titles.project(selection.source.clone(), sub, owner, context)?;
            let base = self.entry(
                (sub, selection),
                &levels,
                template,
                (template_data, broker),
                (
                    &shared,
                    Found {
                        pickup,
                        offered_at,
                        facts,
                        cooldown: cooldown.resolve(sub)?,
                        spawns,
                        property_set_guids,
                    },
                ),
                (context, owner.fields()),
            )?;
            Ok(PendingContract {
                base,
                titles,
                requirements,
            })
        };
        self.append_entries(out, make)
    }

    fn prepare_facts<'fields, 'scope, 'read>(
        &self,
        owner: &ContractOwner<'fields, 'scope, 'read, 'f>,
        context: &MissionEntryContext<'_, 'f>,
    ) -> Result<
        (
            PreparedCooldown<'fields, 'scope, 'read, 'f>,
            PreparedFacts<'fields, 'scope, 'read, 'f>,
        ),
        CatalogError,
    > {
        let fields = owner.fields();
        let cooldown = PreparedCooldown::read(fields, self.handler, self.contract)?;
        let layered = PreparedFacts::read(
            fields,
            (self.handler, self.contract),
            (context.index, context.projection.origins()),
        )?;
        Ok((cooldown, layered))
    }

    fn append_entries(
        &self,
        out: &mut Vec<PendingContract<'f>>,
        make: impl Fn(Option<&'t StructValue<'f>>) -> Result<PendingContract<'f>, ReadError>,
    ) -> Result<usize, ReadError> {
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
        (sub, selection): (Option<&StructValue<'f>>, ContractSelection),
        levels: &Levels<'_, 'f>,
        template: Option<Guid>,
        (template_data, broker): (Option<&Template<'f>>, Option<&Broker<'f>>),
        (shared, found): (&Shared<'f>, Found<'f>),
        (context, fields): (
            &MissionEntryContext<'_, 'f>,
            &OriginalFlowFields<'_, '_, 'f>,
        ),
    ) -> Result<ContractBase<'f>, CatalogError> {
        let Found {
            pickup,
            offered_at,
            facts,
            cooldown,
            spawns,
            property_set_guids,
        } = found;
        let (index, texts) = (context.index, context.texts);
        let faction = self.faction(context);
        let rewards = Rewards::read(
            (self.handler, self.contract, sub),
            faction.as_ref(),
            (index, texts),
            (fields, context.projection.original()),
        )?;
        let mission_type = selected_mission_type(levels, template_data, broker);
        Ok(ContractBase {
            source: selection.source,
            guid: selection.guid,
            sub_contract_guid: selection.sub_contract_guid,
            record_guid: CatalogGuid(self.tree.guid()),
            debug_name: self.name,
            handler: text(Some(self.handler), "debugName"),
            template_guid: template.map(CatalogGuid),
            broker_entry_guid: reference(Some(self.contract), "missionBrokerEntry"),
            not_for_release: boolean(Some(self.contract), "notForRelease"),
            work_in_progress: boolean(Some(self.contract), "workInProgress"),
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
            cooldown,
            facts,
            ship_spawns: spawns.ship_spawns,
            npc_spawns: spawns.npc_spawns,
            entity_spawns: spawns.entity_spawns,
            location_queries: spawns.location_queries,
            property_set_guids,
            location_prerequisites: spawns.location_prerequisites,
            hauling_orders: HaulingLevels::of(levels, template_data, broker).orders(),
            fixed_rewards: shared.fixed_rewards.clone(),
        })
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

struct PreparedTitles<'read, 'source> {
    handler: &'read LayerTexts<'source>,
    contract: LayerTexts<'source>,
    links: TextDependencyLinks,
    template: Option<&'read TemplateTexts<'source>>,
    broker: Option<&'read BrokerTexts<'source>>,
}

impl<'read, 'source> PreparedTitles<'read, 'source> {
    fn read(
        contract: &StructValue<'source>,
        handler: &'read LayerTexts<'source>,
        owner: &ContractOwner<'_, '_, '_, 'source>,
        context: &MissionEntryContext<'_, 'source>,
        template: Option<&'read Template<'source>>,
        broker: Option<&'read Broker<'source>>,
    ) -> Result<Self, CatalogError> {
        let origins = context.projection.origins();
        let contract_texts =
            LayerTexts::read(owner.fields(), origins, contract, LayerKind::Contract)?;
        let links = TextDependencyLinks::read(owner.fields(), origins, contract)?;
        Ok(Self {
            handler,
            contract: contract_texts,
            links,
            template: template.and_then(|template| template.texts.as_ref()),
            broker: broker.and_then(|broker| broker.texts.as_ref()),
        })
    }

    fn project(
        &self,
        occurrence: ContractOccurrence,
        sub: Option<&StructValue<'source>>,
        owner: &ContractOwner<'_, '_, '_, 'source>,
        context: &MissionEntryContext<'_, 'source>,
    ) -> Result<MissionTitles<'source>, CatalogError> {
        let origins = context.projection.origins();
        let sub_texts = sub
            .map(|sub| LayerTexts::read(owner.fields(), origins, sub, LayerKind::SubContract))
            .transpose()?;
        let layers = Layers {
            sub: sub_texts.as_ref(),
            contract: &self.contract,
            handler: self.handler,
        };
        let dependencies = Dependencies {
            template: self.template,
            broker: self.broker,
            links: &self.links,
        };
        let inputs = Inputs {
            projection: context.projection,
            index: context.index,
            cancel: context.cancel,
        };
        mission_titles::titles(occurrence, &layers, &dependencies, &inputs)
    }
}

fn selected_mission_type(
    levels: &Levels<'_, '_>,
    template: Option<&Template<'_>>,
    broker: Option<&Broker<'_>>,
) -> Option<Guid> {
    levels
        .iter()
        .find_map(|level| level.mission_type)
        .or_else(|| template.and_then(|template| template.mission_type))
        .or_else(|| broker.and_then(|broker| broker.mission_type))
}
