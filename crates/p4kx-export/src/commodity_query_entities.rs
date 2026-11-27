// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;

use p4kx_extraction::{CancelToken, DataForge, Record, RecordTree, TreeBuilder};

use super::plan::Plan;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_queries::CatalogRecordCoverage;
use crate::catalog_queries::mining_definitions::dependencies::Dependencies;
use crate::catalog_query_session;
use crate::commodity;
use crate::commodity_authoring_read;
use crate::commodity_backfill::{self, CommodityPresence};
use crate::commodity_contract::Commodity;
use crate::commodity_resource::{self, CommodityResourceSelection};
use crate::commodity_source;
use crate::entity_selection::{self, Selected};
use crate::record_fields;
use crate::resource_group::Groups;
use crate::source_fields::FieldError;
use crate::{CatalogError, ExportAccountingError, Kind};

pub(super) struct Selection<'source> {
    pub(super) forge: &'source DataForge<'source>,
    pub(super) rows: Vec<Commodity<'source>>,
    pub(super) presence: CommodityPresence,
    pub(super) dependency_coverage: CatalogRecordCoverage,
    pub(super) representation_complete: bool,
}

impl<'source> Selection<'source> {
    pub(super) fn read(
        plan: &mut Plan<'source>,
        forge: &'source DataForge<'source>,
        projection: &CatalogProjection<'_, 'source>,
        groups: &mut Dependencies,
        cancel: Option<&CancelToken>,
    ) -> Result<Self, CatalogError> {
        let mut selection = Self {
            forge,
            rows: Vec::new(),
            presence: CommodityPresence::new(),
            dependency_coverage: CatalogRecordCoverage::default(),
            representation_complete: plan.representation_complete,
        };
        let mut trees = TreeBuilder::new();
        for record in &plan.entities {
            catalog_query_session::checkpoint(cancel)?;
            selection.entity(
                record,
                &mut trees,
                projection,
                groups,
                &mut plan.coverage,
                cancel,
            )?;
        }
        if !plan.resources.is_empty() {
            selection.additional_presence(&plan.entities, &mut trees, projection, cancel)?;
        }
        Ok(selection)
    }

    fn entity(
        &mut self,
        record: &Record<'source>,
        trees: &mut TreeBuilder<'source>,
        projection: &CatalogProjection<'_, 'source>,
        groups: &mut Dependencies,
        coverage: &mut CatalogRecordCoverage,
        cancel: Option<&CancelToken>,
    ) -> Result<(), CatalogError> {
        let Some(tree) = read_tree(trees, record, projection, coverage)? else {
            self.representation_complete = false;
            return Ok(());
        };
        let Ok(selected) = commodity_resource::first(&tree) else {
            self.representation_complete = false;
            coverage.incomplete_fields.push(record.index());
            return Ok(());
        };
        let empty = Groups::default();
        let membership = if selected.link.is_some() {
            groups.groups(projection.original(), cancel)?.0
        } else {
            &empty
        };
        let resource = self.linked_resource(&selected, trees, projection)?;
        let mut read =
            commodity::commodity(&tree, projection, resource.as_ref(), membership, &selected)?;
        retain_notes(record.index(), &read.notes, coverage);
        if let Some(resource) = resource {
            retain_notes(
                resource.record_index(),
                &read.resource_notes,
                &mut self.dependency_coverage,
            );
        }
        self.presence
            .observe_entity(&mut read.value, has_crafter(&tree));
        self.rows.push(read.value);
        Ok(())
    }

    fn linked_resource(
        &mut self,
        selected: &CommodityResourceSelection<'_, 'source>,
        trees: &mut TreeBuilder<'source>,
        projection: &CatalogProjection<'_, 'source>,
    ) -> Result<Option<RecordTree<'source>>, CatalogError> {
        let Some(index) = selected.link.as_ref().and_then(|link| {
            if link.reference.target_type.as_deref() != Some("ResourceType") {
                return None;
            }
            link.reference.target_record_index
        }) else {
            return Ok(None);
        };
        let record = self
            .forge
            .record(index)
            .ok_or(ExportAccountingError::MissingPhysicalReferenceTarget { index })?;
        read_tree(trees, &record, projection, &mut self.dependency_coverage)
    }

    fn additional_presence(
        &mut self,
        selected: &[Record<'source>],
        trees: &mut TreeBuilder<'source>,
        projection: &CatalogProjection<'_, 'source>,
        cancel: Option<&CancelToken>,
    ) -> Result<(), CatalogError> {
        let selected: HashSet<_> = selected.iter().map(Record::index).collect();
        for record in self.forge.records() {
            catalog_query_session::checkpoint(cancel)?;
            if selected.contains(&record.index()) {
                continue;
            }
            match entity_selection::select(&record, None) {
                Ok(Some(Selected::Entity(Kind::Commodities))) => {}
                Ok(_) => continue,
                Err(_) => {
                    self.representation_complete &=
                        !commodity_backfill::header_failure_affects_presence(&record);
                    self.dependency_coverage
                        .unreadable_headers
                        .push(record.index());
                    continue;
                }
            }
            if let Some(tree) =
                read_tree(trees, &record, projection, &mut self.dependency_coverage)?
            {
                self.observe_only(&tree, projection)?;
            } else {
                self.representation_complete = false;
            }
        }
        Ok(())
    }

    fn observe_only(
        &mut self,
        tree: &RecordTree<'source>,
        projection: &CatalogProjection<'_, 'source>,
    ) -> Result<(), CatalogError> {
        let Ok(selected) = commodity_resource::first(tree) else {
            self.representation_complete = false;
            self.dependency_coverage
                .incomplete_fields
                .push(tree.record_index());
            return Ok(());
        };
        let origins = projection.origins();
        let links = commodity_source::link_inputs(tree, &selected, origins)?;
        let (inputs, complete) = commodity_authoring_read::crafter_evidence(tree, origins)?;
        let (inputs, complete) =
            commodity::representation_headers(tree, inputs, complete, origins)?;
        self.presence.observe_source(
            selected.link.as_ref().map(|link| link.reference.guid.0),
            links,
            inputs,
            complete,
            has_crafter(tree),
        );
        Ok(())
    }
}

fn has_crafter(tree: &RecordTree<'_>) -> bool {
    record_fields::component(tree, crate::scan::CRAFTER_STRUCT).is_some()
}

pub(super) fn read_tree<'source>(
    trees: &mut TreeBuilder<'source>,
    record: &Record<'source>,
    projection: &CatalogProjection<'_, 'source>,
    coverage: &mut CatalogRecordCoverage,
) -> Result<Option<RecordTree<'source>>, CatalogError> {
    let Ok(tree) = trees.build(record) else {
        coverage.unreadable_bodies.push(record.index());
        return Ok(None);
    };
    projection.original().record_scope(record, &tree)?;
    Ok(Some(tree))
}

pub(super) fn retain_notes(index: u32, notes: &[FieldError], coverage: &mut CatalogRecordCoverage) {
    if notes
        .iter()
        .any(|note| note.description().issue.blocks_projection())
    {
        coverage.incomplete_fields.push(index);
    }
}
