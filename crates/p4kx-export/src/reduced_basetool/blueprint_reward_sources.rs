// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::path::Path;

use p4kx_extraction::{CancelToken, DecimalU64};
use serde::Serialize;

use super::ReducedError;
use super::cell::Cell;
use super::completeness;
use super::mission_titles::ContractPosition;
use super::number::Number;
use super::record_link::RecordLink;
use super::record_targets::RecordTargets;
use super::selected_texts::{SelectedTexts, TextRefs};
use crate::catalog_queries::crafting::{
    BlueprintRewardSelection, BlueprintRewardSnapshot, BlueprintRewardSource,
};
use crate::catalog_queries::{
    CatalogRecordSelection, ContractOccurrence, ContractSnapshot, ParsedCatalogQueries,
};
use crate::catalog_query_session::checkpoint;
use crate::catalog_rows::staged::{StagedCatalogRows, StagedCatalogTable};
use crate::catalog_text_contract::GameTextRef;
use crate::guid_text::CatalogGuid;
use crate::mission_generator::GeneratorDefinition;
use crate::mission_title_origin::ProjectedMissionText;
use crate::{CatalogError, Kind};

#[path = "blueprint_reward_contract_owner.rs"]
mod contract_owner;

#[cfg(test)]
#[path = "blueprint_reward_sources_tests.rs"]
mod tests;

const PATH: &str = "declarations.blueprintRewardSources";
const GENERATOR: &str = "ContractGenerator";

/// Stages every canonical direct pool declaration and closes its selected title reference.
pub(super) fn collect(
    parsed: &ParsedCatalogQueries<'_>,
    contracts: &ContractSnapshot<'_>,
    pools: &RecordTargets,
    scratch: &Path,
    cancel: Option<&CancelToken>,
    texts: &mut SelectedTexts,
) -> Result<StagedCatalogTable, ReducedError> {
    let selection = BlueprintRewardSelection::SourceOwners(CatalogRecordSelection::All);
    parsed.with_blueprint_reward_sources(selection, cancel, |input| {
        stage(&input, contracts, pools, scratch, cancel, texts)
    })?
}

fn stage(
    input: &BlueprintRewardSnapshot<'_>,
    contracts: &ContractSnapshot<'_>,
    pools: &RecordTargets,
    scratch: &Path,
    cancel: Option<&CancelToken>,
    texts: &mut SelectedTexts,
) -> Result<StagedCatalogTable, ReducedError> {
    let availability = input.availability();
    completeness::inventory(PATH, availability, input.source_coverage())?;
    completeness::inventory(PATH, availability, input.pool_coverage())?;
    let contracts_index = Contracts::read(contracts, cancel)?;
    let mut refs = TextRefs::default();
    let mut staged = StagedCatalogRows::new(scratch, cancel);
    for source in input.rows() {
        checkpoint(cancel)?;
        let owner = source.source();
        let record_index = completeness::cell(PATH, owner.record_index())?;
        let source_ordinal = completeness::cell(PATH, source.source_ordinal())?;
        let pool = completeness::cell(PATH, source.pool())?;
        let contract_id = completeness::cell(PATH, source.contract_id())?;
        let match_result = contracts_index.resolve(
            source,
            *record_index.value.ok_or_else(incomplete)?,
            source_ordinal.value.ok_or_else(incomplete)?.get(),
        )?;
        let (contract_status, contract, title) = match match_result {
            ContractMatch::Resolved(resolved) => {
                let key = completeness::cell(PATH, &resolved.title.key)?;
                refs.game_text(key.value);
                (
                    ContractStatus::Resolved,
                    Some(ContractPosition::from(resolved.source)),
                    Some(key),
                )
            }
            ContractMatch::Missing(status) => (status, None, None),
        };
        staged.append(&Row {
            guid: completeness::cell(PATH, &owner.guid)?,
            record_path: completeness::cell(PATH, owner.record_path())?.map(|path| *path),
            record_index,
            source_ordinal,
            contract_id,
            contract_status,
            contract,
            title,
            chance: completeness::cell(PATH, source.chance())?.map(Number::from),
            pool: pool.try_map(|reference| {
                RecordLink::read_source(PATH, reference, pools, |target| {
                    target == Kind::BlueprintPools.records().type_name
                })
            })?,
        })?;
    }
    let table = staged.finish(availability)?;
    completeness::rows(PATH, input.rows().len(), table.len())?;
    texts.add(refs, &[], &contracts.game_texts, cancel)?;
    Ok(table)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Row<'read> {
    guid: Cell<&'read CatalogGuid>,
    record_path: Cell<&'read str>,
    record_index: Cell<&'read u32>,
    source_ordinal: Cell<&'read DecimalU64>,
    contract_id: Cell<&'read CatalogGuid>,
    contract_status: ContractStatus,
    contract: Option<ContractPosition>,
    title: Option<Cell<&'read GameTextRef>>,
    chance: Cell<Number<'read>>,
    pool: Cell<RecordLink<'read>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
enum ContractStatus {
    Resolved,
    OutsideGenerator,
    NoAuthoredId,
    Unpublished,
}

enum ContractMatch<'read, 'source> {
    Resolved(&'read Published<'read, 'source>),
    Missing(ContractStatus),
}

struct Published<'read, 'source> {
    source: &'read ContractOccurrence,
    id: CatalogGuid,
    title: &'read ProjectedMissionText<'source>,
}

struct Contracts<'read, 'source> {
    generators: HashMap<u32, &'read GeneratorDefinition<'source>>,
    published: HashMap<(u32, u64), Published<'read, 'source>>,
}

impl<'read, 'source> Contracts<'read, 'source> {
    fn read(
        snapshot: &'read ContractSnapshot<'source>,
        cancel: Option<&CancelToken>,
    ) -> Result<Self, ReducedError> {
        let mut generators = HashMap::new();
        let mut published = HashMap::new();
        generators
            .try_reserve(snapshot.generators.len())
            .map_err(CatalogError::SourceUseAllocation)?;
        published
            .try_reserve(snapshot.rows.len())
            .map_err(CatalogError::SourceUseAllocation)?;
        for generator in &snapshot.generators {
            checkpoint(cancel)?;
            let index = generator
                .header()
                .record_index()
                .value()
                .ok_or_else(incomplete)?;
            if generators.insert(*index, generator).is_some() {
                return Err(incomplete());
            }
        }
        completeness::rows(PATH, snapshot.rows.len(), snapshot.mission_titles.len())?;
        for (row, titles) in snapshot.rows.iter().zip(&snapshot.mission_titles) {
            checkpoint(cancel)?;
            let source = &row.base.source;
            if source.input_ref() != titles.contract.input_ref()
                || ContractPosition::from(source) != ContractPosition::from(&titles.contract)
                || generators
                    .get(&source.record_index())
                    .is_none_or(|generator| generator.input_ref() != source.input_ref())
            {
                return Err(incomplete());
            }
            let (position, id) = source.sub_contract().map_or_else(
                || (source.contract(), &row.base.guid),
                |sub| (sub, &row.base.sub_contract_guid),
            );
            let key = (source.record_index(), position.value_index());
            let id = *completeness::cell(PATH, id)?.value.ok_or_else(incomplete)?;
            if published
                .insert(
                    key,
                    Published {
                        source,
                        id,
                        title: &titles.title,
                    },
                )
                .is_some()
            {
                return Err(incomplete());
            }
        }
        Ok(Self {
            generators,
            published,
        })
    }

    fn resolve(
        &self,
        reward: &BlueprintRewardSource<'_>,
        index: u32,
        ordinal: u64,
    ) -> Result<ContractMatch<'_, 'source>, ReducedError> {
        if reward.source().source_type().value() != Some(&GENERATOR) {
            return Ok(ContractMatch::Missing(ContractStatus::OutsideGenerator));
        }
        let Some(id) = reward.contract_id().value() else {
            return Ok(ContractMatch::Missing(ContractStatus::NoAuthoredId));
        };
        let Some(generator) = self.generators.get(&index) else {
            return Ok(ContractMatch::Missing(ContractStatus::Unpublished));
        };
        if generator.header().guid.value() != reward.source().guid.value()
            || generator.header().record_path().value() != reward.source().record_path().value()
        {
            return Err(incomplete());
        }
        let pool = reward.pool().value().ok_or_else(incomplete)?;
        let position = contract_owner::nearest(generator.source(), ordinal, id, pool)?;
        let row = position.and_then(|position| self.published.get(&(index, position)));
        match row {
            Some(row) if &row.id == id => Ok(ContractMatch::Resolved(row)),
            Some(_) => Err(incomplete()),
            None => Ok(ContractMatch::Missing(ContractStatus::Unpublished)),
        }
    }
}

const fn incomplete() -> ReducedError {
    ReducedError::IncompleteScope(PATH)
}
