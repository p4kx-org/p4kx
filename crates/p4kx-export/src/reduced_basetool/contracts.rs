// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::{CancelToken, SourcedValue};
use serde::Serialize;

use super::ReducedError;
use super::cell::Cell;
use super::completeness;
use super::mission_titles::{self, ContractPosition};
use super::selected_texts::{SelectedTexts, TextRefs};
use crate::catalog_queries::{
    BlueprintReward, ContractBase, ContractFaction, ContractSnapshot, FixedReward, GiverFaction,
    GiverSource, LocationPrerequisite, LocationQueryLink, MissionTitles, OfferedAt, PickupPlace,
    ReputationGain, StandingRequirement,
};
use crate::catalog_query_session::checkpoint;
use crate::catalog_rows::staged::{StagedCatalogRows, StagedCatalogTable};
use crate::catalog_text_contract::GameTextRef;
use crate::guid_text::CatalogGuid;
use crate::resource_integer::ResourceInt64;

#[path = "contract_terms.rs"]
mod terms;

use self::terms::Terms;

pub(super) const PATH: &str = "projections.contracts";

pub(super) struct Contracts {
    pub(super) rows: StagedCatalogTable,
    pub(super) titles: StagedCatalogTable,
}

pub(super) fn stage(
    source: &ContractSnapshot<'_>,
    scratch: &Path,
    cancel: Option<&CancelToken>,
    texts: &mut SelectedTexts,
) -> Result<Contracts, ReducedError> {
    let titles = mission_titles::collect(source, scratch, cancel, texts)?;
    if !source.coverage.dependencies.forward_failures.is_empty() {
        return Err(ReducedError::IncompleteScope(PATH));
    }
    let mut rows = StagedCatalogRows::new(scratch, cancel);
    let mut references = TextRefs::default();
    for (input, original_title) in source.rows.iter().zip(&source.mission_titles) {
        checkpoint(cancel)?;
        rows.append(&Contract::read(
            &input.base,
            original_title,
            &mut references,
        )?)?;
    }
    let rows = rows.finish(source.availability)?;
    completeness::rows(PATH, source.rows.len(), rows.len())?;
    texts.add(references, &[], &source.game_texts, cancel)?;
    Ok(Contracts { rows, titles })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Contract<'read, 'source> {
    source: ContractPosition,
    guid: Cell<&'read CatalogGuid>,
    sub_contract_guid: Cell<&'read CatalogGuid>,
    debug_name: Option<&'source str>,
    handler: Option<&'source str>,
    title: Text<'read>,
    mission_type_guid: Option<CatalogGuid>,
    mission_type: Option<Text<'read>>,
    contractor: Text<'read>,
    faction: Option<Faction<'read, 'source>>,
    giver_faction: Option<Giver<'read, 'source>>,
    pickup_places: Vec<Place<'read, 'source>>,
    offered_at: Offer<'read, 'source>,
    #[serde(flatten)]
    rewards: Rewards<'read>,
    min_standing_guid: Option<CatalogGuid>,
    max_standing_guid: Option<CatalogGuid>,
    standing_requirements: &'read [StandingRequirement],
    #[serde(flatten)]
    terms: Terms<'read>,
    not_for_release: Option<bool>,
    work_in_progress: Option<bool>,
    template_guid: Option<CatalogGuid>,
    broker_entry_guid: Option<CatalogGuid>,
    location_queries: &'read [LocationQueryLink],
    location_prerequisites: &'read [LocationPrerequisite],
}

impl<'read, 'source> Contract<'read, 'source> {
    fn read(
        row: &'read ContractBase<'source>,
        titles: &'read MissionTitles<'_>,
        references: &mut TextRefs<'read>,
    ) -> Result<Self, ReducedError> {
        Ok(Self {
            source: ContractPosition::from(&row.source),
            guid: completeness::cell(PATH, &row.guid)?,
            sub_contract_guid: completeness::cell(PATH, &row.sub_contract_guid)?,
            debug_name: row.debug_name,
            handler: row.handler,
            title: text(&titles.title.key, references)?,
            mission_type_guid: row.mission_type_guid,
            mission_type: optional_text(row.mission_type.as_ref(), references)?,
            contractor: text(&row.contractor, references)?,
            faction: row
                .faction
                .as_ref()
                .map(|value| Faction::read(value, references))
                .transpose()?,
            giver_faction: row
                .giver_faction
                .as_ref()
                .map(|value| Giver::read(value, references))
                .transpose()?,
            pickup_places: places(&row.pickup_places, references)?,
            offered_at: Offer::read(&row.offered_at, references)?,
            rewards: Rewards::read(row)?,
            min_standing_guid: row.min_standing_guid,
            max_standing_guid: row.max_standing_guid,
            standing_requirements: &row.standing_requirements,
            terms: Terms::read(&row.cooldown, &row.facts)?,
            not_for_release: row.not_for_release,
            work_in_progress: row.work_in_progress,
            template_guid: row.template_guid,
            broker_entry_guid: row.broker_entry_guid,
            location_queries: &row.location_queries,
            location_prerequisites: &row.location_prerequisites,
        })
    }
}

type Text<'read> = Cell<&'read GameTextRef>;

fn text<'read>(
    key: &'read SourcedValue<GameTextRef>,
    references: &mut TextRefs<'read>,
) -> Result<Text<'read>, ReducedError> {
    let cell = completeness::cell(PATH, key)?;
    references.game_text(cell.value);
    Ok(cell)
}

fn optional_text<'read>(
    key: Option<&'read SourcedValue<GameTextRef>>,
    references: &mut TextRefs<'read>,
) -> Result<Option<Text<'read>>, ReducedError> {
    key.map(|value| text(value, references)).transpose()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Faction<'read, 'source> {
    guid: CatalogGuid,
    name: &'source str,
    display_name: Option<Text<'read>>,
}

impl<'read, 'source> Faction<'read, 'source> {
    fn read(
        row: &'read ContractFaction<'source>,
        references: &mut TextRefs<'read>,
    ) -> Result<Self, ReducedError> {
        Ok(Self {
            guid: row.guid,
            name: row.name,
            display_name: optional_text(row.display_name.as_ref(), references)?,
        })
    }
}

#[derive(Serialize)]
struct Giver<'read, 'source> {
    source: GiverSource,
    faction: Faction<'read, 'source>,
}

impl<'read, 'source> Giver<'read, 'source> {
    fn read(
        row: &'read GiverFaction<'source>,
        references: &mut TextRefs<'read>,
    ) -> Result<Self, ReducedError> {
        Ok(Self {
            source: row.source,
            faction: Faction::read(&row.faction, references)?,
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Offer<'read, 'source> {
    locality_guids: &'read [CatalogGuid],
    locations: Vec<Place<'read, 'source>>,
    broker_fallback: bool,
}

impl<'read, 'source> Offer<'read, 'source> {
    fn read(
        offer: &'read OfferedAt<'source>,
        references: &mut TextRefs<'read>,
    ) -> Result<Self, ReducedError> {
        Ok(Self {
            locality_guids: &offer.locality_guids,
            locations: places(&offer.locations, references)?,
            broker_fallback: offer.broker_fallback,
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Place<'read, 'source> {
    starmap_object_guid: CatalogGuid,
    object_type: Option<&'source str>,
    #[serde(rename = "place")]
    label: Option<Text<'read>>,
}

fn places<'read, 'source>(
    rows: &'read [PickupPlace<'source>],
    references: &mut TextRefs<'read>,
) -> Result<Vec<Place<'read, 'source>>, ReducedError> {
    rows.iter()
        .map(|row| {
            Ok(Place {
                starmap_object_guid: row.starmap_object_guid,
                object_type: row.object_type,
                label: optional_text(row.place.as_ref(), references)?,
            })
        })
        .collect()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Rewards<'read> {
    #[serde(rename = "blueprintRewards")]
    blueprints: &'read [BlueprintReward],
    #[serde(rename = "fixedRewards")]
    fixed: Cell<Vec<Payout<'read>>>,
    reputation_gains: Cell<Vec<Reputation<'read>>>,
}

impl<'read> Rewards<'read> {
    fn read(row: &'read ContractBase<'_>) -> Result<Self, ReducedError> {
        Ok(Self {
            blueprints: &row.blueprint_rewards,
            fixed: completeness::cell(PATH, &row.fixed_rewards)?
                .try_map(|rows| rows.iter().map(Payout::read).collect::<Result<_, _>>())?,
            reputation_gains: completeness::cell(PATH, &row.reputation_gains)?
                .try_map(|rows| rows.iter().map(Reputation::read).collect::<Result<_, _>>())?,
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Payout<'read> {
    currency: Cell<&'read str>,
    amount: Cell<&'read ResourceInt64>,
    max: Cell<&'read ResourceInt64>,
    plus_bonuses: Cell<&'read bool>,
}

impl<'read> Payout<'read> {
    fn read(row: &'read FixedReward<'_>) -> Result<Self, ReducedError> {
        Ok(Self {
            currency: completeness::cell(PATH, &row.currency)?.map(|value| *value),
            amount: completeness::cell(PATH, &row.amount)?,
            max: completeness::cell(PATH, &row.max)?,
            plus_bonuses: completeness::cell(PATH, &row.plus_bonuses)?,
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Reputation<'read> {
    faction_reputation_guid: Cell<&'read CatalogGuid>,
    faction_reputation_name: Cell<&'read str>,
    scope_guid: Cell<&'read CatalogGuid>,
    scope_name: Cell<&'read str>,
    amount: Cell<&'read ResourceInt64>,
}

impl<'read> Reputation<'read> {
    fn read(row: &'read ReputationGain<'_>) -> Result<Self, ReducedError> {
        Ok(Self {
            faction_reputation_guid: completeness::cell(PATH, &row.faction_reputation_guid)?,
            faction_reputation_name: completeness::cell(PATH, &row.faction_reputation_name)?
                .map(|value| *value),
            scope_guid: completeness::cell(PATH, &row.scope_guid)?,
            scope_name: completeness::cell(PATH, &row.scope_name)?.map(|value| *value),
            amount: completeness::cell(PATH, &row.amount)?,
        })
    }
}
