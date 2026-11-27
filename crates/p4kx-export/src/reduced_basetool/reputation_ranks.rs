// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::{CancelToken, SourcedValue};
use serde::Serialize;

use super::ReducedError;
use super::cell::Cell;
use super::completeness;
use super::number::Number;
use super::record_identity::RecordIdentity;
use super::record_link::RecordLink;
use super::record_targets::RecordTargets;
use super::selected_texts::TextRefs;

use crate::catalog_queries::{ReputationRows, ReputationSnapshot};
use crate::catalog_rows::staged::{StagedCatalogRows, StagedCatalogTable};
use crate::catalog_text_contract::GameTextRef;
use crate::catalog_v3::reputation_scope_definition::{ScopeDefinition, ScopeStandingMap};
use crate::catalog_v3::reputation_standing_definition::StandingDefinition;
use crate::resource_float::ResourceFloat32;
use crate::resource_number::ResourceNumber;

pub(super) const SCOPES: &str = "definitions.reputationScopes";
pub(super) const STANDINGS: &str = "definitions.reputationStandings";

pub(super) struct Ranks {
    pub(super) scopes: StagedCatalogTable,
    pub(super) scope_targets: RecordTargets,
    pub(super) standings: StagedCatalogTable,
    pub(super) standing_targets: RecordTargets,
}

pub(super) fn stage<'read>(
    snapshot: &'read ReputationSnapshot<'_>,
    scratch: &Path,
    cancel: Option<&CancelToken>,
    references: &mut TextRefs<'read>,
) -> Result<Ranks, ReducedError> {
    let source_scopes = snapshot
        .scopes
        .as_ref()
        .ok_or(ReducedError::IncompleteScope(SCOPES))?;
    let source_standings = snapshot
        .standings
        .as_ref()
        .ok_or(ReducedError::IncompleteScope(STANDINGS))?;
    completeness::inventory(SCOPES, source_scopes.availability, &source_scopes.coverage)?;
    completeness::inventory(
        STANDINGS,
        source_standings.availability,
        &source_standings.coverage,
    )?;

    let (standings, standing_targets) =
        stage_standings(source_standings, scratch, cancel, references)?;
    let mut scopes = StagedCatalogRows::new(scratch, cancel);
    let mut scope_targets = RecordTargets::default();
    for row in &source_scopes.rows {
        scope_targets.insert(SCOPES, row.source())?;
        scopes.append(&Scope::read(row, &standing_targets, references)?)?;
    }
    let scopes = scopes.finish(source_scopes.availability)?;
    completeness::rows(SCOPES, source_scopes.rows.len(), scopes.len())?;
    Ok(Ranks {
        scopes,
        scope_targets,
        standings,
        standing_targets,
    })
}

fn stage_standings<'read>(
    source: &'read ReputationRows<StandingDefinition<'_>>,
    scratch: &Path,
    cancel: Option<&CancelToken>,
    references: &mut TextRefs<'read>,
) -> Result<(StagedCatalogTable, RecordTargets), ReducedError> {
    let mut output = StagedCatalogRows::new(scratch, cancel);
    let mut targets = RecordTargets::default();
    for original in &source.rows {
        let row = Standing::read(original, references)?;
        targets.insert(STANDINGS, original.source())?;
        output.append(&row)?;
    }
    let standings = output.finish(source.availability)?;
    completeness::rows(STANDINGS, source.rows.len(), standings.len())?;
    Ok((standings, targets))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Scope<'read> {
    #[serde(flatten)]
    identity: RecordIdentity<'read>,
    #[serde(rename = "scopeName")]
    name: Cell<&'read str>,
    display_name: Cell<&'read GameTextRef>,
    standing_map: Cell<StandingMap<'read>>,
}

impl<'read> Scope<'read> {
    fn read(
        source: &'read ScopeDefinition<'_>,
        targets: &RecordTargets,
        references: &mut TextRefs<'read>,
    ) -> Result<Self, ReducedError> {
        let header = source.source();
        completeness::value(SCOPES, &header.guid())?;
        completeness::value(SCOPES, header.record_index())?;
        completeness::value(SCOPES, header.class_name())?;
        completeness::value(SCOPES, header.record_path())?;
        let display_name = completeness::cell(SCOPES, source.display_name())?;
        references.game_text(display_name.value);
        Ok(Self {
            identity: header.into(),
            name: completeness::cell(SCOPES, source.scope_name())?.map(|name| *name),
            display_name,
            standing_map: completeness::cell(SCOPES, source.standing_map())?
                .try_map(|map| StandingMap::read(map, targets))?,
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StandingMap<'read> {
    initial_reputation: Cell<Number<'read>>,
    reputation_ceiling: Cell<Number<'read>>,
    standings: Cell<Vec<Cell<RecordLink<'read>>>>,
}

impl<'read> StandingMap<'read> {
    fn read(
        source: &'read ScopeStandingMap<'_>,
        targets: &RecordTargets,
    ) -> Result<Self, ReducedError> {
        Ok(Self {
            initial_reputation: number(SCOPES, source.initial_reputation())?,
            reputation_ceiling: number(SCOPES, source.reputation_ceiling())?,
            standings: completeness::list(SCOPES, source.standings(), |link| {
                RecordLink::read_source(SCOPES, link, targets, |_| true)
            })?,
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Standing<'read> {
    #[serde(flatten)]
    identity: RecordIdentity<'read>,
    name: Cell<&'read str>,
    display_name: Cell<&'read GameTextRef>,
    perk_description: Cell<&'read GameTextRef>,
    min_reputation: Cell<Number<'read>>,
    gated: Cell<&'read bool>,
    drift_reputation: Cell<Number<'read>>,
    drift_time_hours: Cell<Number<'static>>,
}

impl<'read> Standing<'read> {
    fn read(
        source: &'read StandingDefinition<'_>,
        references: &mut TextRefs<'read>,
    ) -> Result<Self, ReducedError> {
        let header = source.source();
        completeness::value(STANDINGS, header.class_name())?;
        completeness::value(STANDINGS, header.record_path())?;
        let presentation = source.presentation();
        let progression = source.progression();
        let display_name = completeness::cell(STANDINGS, presentation.display_name())?;
        let perk_description = completeness::cell(STANDINGS, presentation.perk_description())?;
        references.game_text(display_name.value);
        references.game_text(perk_description.value);
        Ok(Self {
            identity: header.into(),
            name: completeness::cell(STANDINGS, presentation.name())?.map(|name| *name),
            display_name,
            perk_description,
            min_reputation: number(STANDINGS, progression.min_reputation())?,
            gated: completeness::cell(STANDINGS, progression.gated())?,
            drift_reputation: number(STANDINGS, progression.drift_reputation())?,
            drift_time_hours: single(progression.drift_time_hours())?,
        })
    }
}

fn number<'read>(
    path: &'static str,
    value: &'read SourcedValue<ResourceNumber>,
) -> Result<Cell<Number<'read>>, ReducedError> {
    Ok(completeness::cell(path, value)?.map(Number::from))
}

fn single(value: &SourcedValue<ResourceFloat32>) -> Result<Cell<Number<'static>>, ReducedError> {
    Ok(completeness::cell(STANDINGS, value)?.map(Number::from))
}
