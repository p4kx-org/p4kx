// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;

use p4kx_extraction::{CancelToken, Record, TreeBuilder};

use super::entities::{self, Selection};
use crate::CatalogError;
use crate::blueprint_backfill_source::BlueprintResources;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_queries::CatalogRecordCoverage;
use crate::catalog_queries::mining_definitions::dependencies::Dependencies;
use crate::catalog_query_session;
use crate::commodity;
use crate::commodity_backfill;
use crate::crafting::ledger::CommodityDependencies;

pub(super) fn append<'source>(
    mut records: Vec<Record<'source>>,
    coverage: &mut CatalogRecordCoverage,
    selection: &mut Selection<'source>,
    projection: &CatalogProjection<'_, 'source>,
    groups: &mut Dependencies,
    cancel: Option<&CancelToken>,
) -> Result<CatalogRecordCoverage, CatalogError> {
    records.retain(|record| !selection.presence.represents(&record.guid()));
    if !records.is_empty() {
        let wanted: HashSet<_> = records.iter().map(Record::guid).collect();
        let sources = CommodityDependencies::read(selection.forge, projection, &wanted, cancel)?;
        super::append_coverage(&mut selection.dependency_coverage, sources.coverage);
        let admitted: HashSet<_> = sources
            .blueprints
            .iter()
            .flat_map(BlueprintResources::targets)
            .chain(sources.quality.targets())
            .collect();
        records.retain(|record| admitted.contains(&record.guid()));
        if !records.is_empty() {
            let (membership, group_coverage) = groups.groups(projection.original(), cancel)?;
            let origins = projection.origins();
            let complete =
                selection.representation_complete && group_coverage.unreadable_bodies.is_empty();
            let presence = std::mem::replace(
                &mut selection.presence,
                commodity_backfill::CommodityPresence::new(),
            )
            .finish(complete, origins);
            let mut trees = TreeBuilder::new();
            for record in &records {
                catalog_query_session::checkpoint(cancel)?;
                let Some(tree) = entities::read_tree(&mut trees, record, projection, coverage)?
                else {
                    continue;
                };
                let choice = commodity_backfill::for_resource(
                    &tree,
                    &presence,
                    &sources.blueprints,
                    &sources.quality,
                    origins,
                )?;
                let row = commodity::resource_commodity(&tree, projection, membership, choice)?;
                entities::retain_notes(record.index(), &row.resource_notes, coverage);
                selection.rows.push(row.value);
            }
        }
    }
    let mut dependency_coverage = std::mem::take(&mut selection.dependency_coverage);
    super::append_coverage(
        &mut dependency_coverage,
        std::mem::take(&mut groups.coverage),
    );
    Ok(dependency_coverage)
}
