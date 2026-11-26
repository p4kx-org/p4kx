// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::CancelToken;
use serde::Serialize;

use super::ReducedError;
use super::cell::Cell;
use super::completeness;
use super::number::Number;
use super::record_identity::RecordIdentity;
use super::record_link::RecordLink;
use super::record_targets::RecordTargets;
use crate::Kind;
use crate::catalog_queries::crafting::{BlueprintPoolEntry, BlueprintPoolSnapshot};
use crate::catalog_queries::{CatalogRecordSelection, ParsedCatalogQueries};
use crate::catalog_rows::staged::{StagedCatalogRows, StagedCatalogTable};

use crate::guid_text::CatalogGuid;

pub(super) const CATEGORIES: &str = "definitions.blueprintCategories";
pub(super) const POOLS: &str = "definitions.blueprintPools";
pub(super) const WEIGHTS: &str = "projections.blueprintPoolWeights";

pub(super) struct Pools {
    pub(super) definitions: StagedCatalogTable,
    pub(super) weights: StagedCatalogTable,
}

pub(super) fn collect_categories(
    parsed: &ParsedCatalogQueries<'_>,
    scratch: &Path,
    cancel: Option<&CancelToken>,
) -> Result<StagedCatalogTable, ReducedError> {
    let snapshot = parsed.read_blueprint_categories(CatalogRecordSelection::All, cancel)?;
    completeness::inventory(CATEGORIES, snapshot.availability, &snapshot.coverage)?;
    let mut staged = StagedCatalogRows::new(scratch, cancel);
    for category in &snapshot.rows {
        let source = &category.source;
        completeness::value(CATEGORIES, &source.guid)?;
        completeness::value(CATEGORIES, &source.record_index)?;
        completeness::value(CATEGORIES, &source.class_name)?;
        completeness::value(CATEGORIES, &source.record_path)?;
        staged.append(&RecordIdentity::from(source))?;
    }
    let table = staged.finish(snapshot.availability)?;
    completeness::rows(CATEGORIES, snapshot.rows.len(), table.len())?;
    Ok(table)
}

pub(super) fn collect_pools(
    parsed: &ParsedCatalogQueries<'_>,
    scratch: &Path,
    cancel: Option<&CancelToken>,
    blueprints: &RecordTargets,
) -> Result<Pools, ReducedError> {
    parsed.with_blueprint_pools(CatalogRecordSelection::All, cancel, |snapshot| {
        stage_pools(&snapshot, scratch, cancel, blueprints)
    })?
}

fn stage_pools(
    snapshot: &BlueprintPoolSnapshot<'_>,
    scratch: &Path,
    cancel: Option<&CancelToken>,
    blueprints: &RecordTargets,
) -> Result<Pools, ReducedError> {
    let availability = snapshot.availability();
    completeness::inventory(POOLS, availability, snapshot.coverage())?;
    completeness::rows(WEIGHTS, snapshot.rows().len(), snapshot.weights().len())?;
    let mut definitions = StagedCatalogRows::new(scratch, cancel);
    let mut weights = StagedCatalogRows::new(scratch, cancel);
    for (definition, weight) in snapshot.rows().iter().zip(snapshot.weights()) {
        let pool = weight.pool();
        if pool.record_index().value() != definition.source().record_index().value()
            || pool.guid().value() != definition.source().guid.value()
        {
            return Err(ReducedError::IncompleteScope(WEIGHTS));
        }
        definitions.append(&Definition {
            identity: definition.source().into(),
            blueprint_rewards: completeness::list(POOLS, definition.blueprint_rewards(), |slot| {
                Reward::read(slot, blueprints)
            })?,
        })?;
        weights.append(&Weight {
            pool: PoolRef {
                record_index: completeness::cell(WEIGHTS, pool.record_index())?,
                guid: completeness::cell(WEIGHTS, pool.guid())?,
            },
            total_weight: completeness::cell(WEIGHTS, weight.total_weight())?.map(Number::from),
        })?;
    }
    let definitions = definitions.finish(availability)?;
    let weights = weights.finish(availability)?;
    completeness::rows(POOLS, snapshot.rows().len(), definitions.len())?;
    completeness::rows(WEIGHTS, definitions.len(), weights.len())?;
    Ok(Pools {
        definitions,
        weights,
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Definition<'read> {
    #[serde(flatten)]
    identity: RecordIdentity<'read>,
    blueprint_rewards: Cell<Vec<Cell<Reward<'read>>>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Reward<'read> {
    blueprint_record: Cell<RecordLink<'read>>,
    weight: Cell<Number<'read>>,
}

impl<'read> Reward<'read> {
    fn read(
        value: &'read BlueprintPoolEntry<'_>,
        targets: &RecordTargets,
    ) -> Result<Self, ReducedError> {
        Ok(Self {
            blueprint_record: completeness::cell(POOLS, value.blueprint_record())?.try_map(
                |reference| {
                    RecordLink::read_source(POOLS, reference, targets, |source_type| {
                        source_type == Kind::Blueprints.records().type_name
                    })
                },
            )?,
            weight: completeness::cell(POOLS, value.weight())?.map(Number::from),
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Weight<'read> {
    pool: PoolRef<'read>,
    total_weight: Cell<Number<'read>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PoolRef<'read> {
    record_index: Cell<&'read u32>,
    guid: Cell<&'read CatalogGuid>,
}
