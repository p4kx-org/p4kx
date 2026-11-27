// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::{CancelToken, SourcedValue};
use serde::Serialize;

use super::ReducedError;
use super::cell::Cell;
use super::completeness;
use super::record_identity::RecordIdentity;
use super::record_link::RecordLink;
use super::record_targets::RecordTargets;
use super::selected_texts::TextRefs;
use crate::catalog_queries::{
    FactionReputationDefinition, FactionText, GameTextRef, RelationMarker, ReputationRelation,
    ReputationRows,
};
use crate::catalog_rows::staged::{StagedCatalogRows, StagedCatalogTable};
use crate::source_value_catalog::SourceRecordReference;

pub(super) const PATH: &str = "definitions.factionReputations";

pub(super) struct Dependencies<'staged> {
    pub(super) scopes: &'staged RecordTargets,
    pub(super) standings: &'staged RecordTargets,
    pub(super) contexts: &'staged RecordTargets,
    pub(super) perks: &'staged RecordTargets,
}

pub(super) struct FactionReputations {
    pub(super) table: StagedCatalogTable,
    pub(super) targets: RecordTargets,
}

pub(super) fn stage<'read>(
    source: &'read ReputationRows<FactionReputationDefinition<'_>>,
    dependencies: &Dependencies<'_>,
    scratch: &Path,
    cancel: Option<&CancelToken>,
    references: &mut TextRefs<'read>,
) -> Result<FactionReputations, ReducedError> {
    completeness::inventory(PATH, source.availability, &source.coverage)?;
    let mut targets = RecordTargets::default();
    for original in &source.rows {
        crate::catalog_query_session::checkpoint(cancel)?;
        let header = original.source();
        completeness::value(PATH, &header.guid())?;
        completeness::value(PATH, header.record_index())?;
        completeness::value(PATH, header.class_name())?;
        completeness::value(PATH, header.record_path())?;
        targets.insert(PATH, header)?;
    }
    let mut output = StagedCatalogRows::new(scratch, cancel);
    for original in &source.rows {
        output.append(&FactionReputation::read(
            original,
            &targets,
            dependencies,
            references,
        )?)?;
    }
    let table = output.finish(source.availability)?;
    completeness::rows(PATH, source.rows.len(), table.len())?;
    Ok(FactionReputations { table, targets })
}

type LinkCell<'read> = Cell<RecordLink<'read>>;
type LinkList<'read> = Cell<Vec<LinkCell<'read>>>;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FactionReputation<'read> {
    #[serde(flatten)]
    identity: RecordIdentity<'read>,
    name: Cell<&'read str>,
    display_name: Cell<&'read GameTextRef>,
    is_npc: Cell<&'read bool>,
    reputation_context_properties_ui: LinkCell<'read>,
    allies: LinkList<'read>,
    enemies: LinkList<'read>,
    hostility_params: Cell<StandingRule<'read>>,
    allied_params: Cell<StandingRule<'read>>,
    perk_reputation_reward_list: LinkList<'read>,
}

impl<'read> FactionReputation<'read> {
    fn read(
        source: &'read FactionReputationDefinition<'_>,
        factions: &RecordTargets,
        dependencies: &Dependencies<'_>,
        references: &mut TextRefs<'read>,
    ) -> Result<Self, ReducedError> {
        let display_name = completeness::cell(PATH, source.display_name())?;
        references.game_text(display_name.value);
        let relation = source.relations();
        Ok(Self {
            identity: source.source().into(),
            name: completeness::cell(PATH, source.name())?.map(|name| *name),
            display_name,
            is_npc: completeness::cell(PATH, source.presentation().is_npc())?,
            reputation_context_properties_ui: link(
                relation.reputation_context_properties_ui(),
                dependencies.contexts,
            )?,
            allies: links(relation.allies(), factions)?,
            enemies: links(relation.enemies(), factions)?,
            hostility_params: completeness::cell(PATH, relation.hostility_params())?
                .try_map(|rule| StandingRule::read(rule, dependencies, references))?,
            allied_params: completeness::cell(PATH, relation.allied_params())?
                .try_map(|rule| StandingRule::read(rule, dependencies, references))?,
            perk_reputation_reward_list: links(
                relation.perk_reputation_reward_list(),
                dependencies.perks,
            )?,
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StandingRule<'read> {
    source_type: Cell<&'read str>,
    scope: LinkCell<'read>,
    standing: LinkCell<'read>,
    marker_params: Cell<Marker<'read>>,
}

impl<'read> StandingRule<'read> {
    fn read(
        source: &'read ReputationRelation<'_>,
        dependencies: &Dependencies<'_>,
        references: &mut TextRefs<'read>,
    ) -> Result<Self, ReducedError> {
        Ok(Self {
            source_type: completeness::cell(PATH, source.source_type())?.map(|source| *source),
            scope: link(source.scope(), dependencies.scopes)?,
            standing: link(source.standing(), dependencies.standings)?,
            marker_params: completeness::cell(PATH, source.marker_params())?
                .try_map(|marker| Marker::read(marker, references))?,
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Marker<'read> {
    source_type: Cell<&'read str>,
    description: Cell<&'read FactionText<'read, GameTextRef>>,
    icon: Cell<&'read str>,
}

impl<'read> Marker<'read> {
    fn read(
        source: &'read RelationMarker<'_>,
        references: &mut TextRefs<'read>,
    ) -> Result<Self, ReducedError> {
        let description = completeness::cell(PATH, source.description())?;
        if let Some(FactionText::Localized { reference }) = description.value {
            references.game_text(Some(reference));
        }
        Ok(Self {
            source_type: completeness::cell(PATH, source.source_type())?.map(|source| *source),
            description,
            icon: completeness::cell(PATH, source.icon())?.map(|icon| *icon),
        })
    }
}

type SourceLink<'source> = SourcedValue<Box<SourceRecordReference<'source>>>;

fn link<'read>(
    source: &'read SourceLink<'_>,
    targets: &RecordTargets,
) -> Result<LinkCell<'read>, ReducedError> {
    completeness::cell(PATH, source)?
        .try_map(|link| RecordLink::read_source(PATH, link, targets, |_| true))
}

fn links<'read>(
    source: &'read SourcedValue<Vec<SourceLink<'_>>>,
    targets: &RecordTargets,
) -> Result<LinkList<'read>, ReducedError> {
    completeness::list(PATH, source, |link| {
        RecordLink::read_source(PATH, link, targets, |_| true)
    })
}
