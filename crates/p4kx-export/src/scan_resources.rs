// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeSet, HashSet};

use p4kx_extraction::{CancelToken, Guid, Record, RecordTree, SharedFieldOrigin, TreeBuilder};

use super::projection::SelectedProjection;
use super::{RESOURCE_TYPE, Scan};
use crate::blueprint_catalog_rows::BlueprintRow;
use crate::catalog_v3::catalog_resource_network_read::resource_network_global;
use crate::catalog_v3::tag_catalog::EntityTags;
use crate::collected::Collected;
use crate::commodity;
use crate::commodity_backfill::{self, CommodityPresence};
use crate::commodity_resource::{self, CommodityResource};
use crate::export_diagnostic::{ExportDiagnosticCode, ExportSourceRole};
use crate::export_ledger::{DerivedFailure, ExportLedger, SourceDiagnosis, SourceDisposition};
use crate::picked::Picked;
use crate::record_observation::RecordObservation;
use crate::scan_health::{ScanError, SourceTrace};
use crate::source_fields::FieldError;
use crate::{CatalogError, ExportAccountingError, Kind};

struct ResourceRead<'source> {
    record: Record<'source>,
    tree: RecordTree<'source>,
}

struct BackfillContext<'context, 'source> {
    representation: SharedFieldOrigin,
    collected: &'context Collected<'source>,
}

#[derive(Default)]
pub(super) struct ResourceRows<'source> {
    pub(super) picked: Vec<Picked<'source>>,
    pub(super) entity_tags: Vec<EntityTags<'source>>,
    pub(super) ledger: ExportLedger,
}
impl<'f> SelectedProjection<'_, '_, '_, 'f> {
    pub(super) fn resource_network_global(
        &self,
        admission: &RecordObservation<'_, 'f>,
        tree: &RecordTree<'f>,
    ) -> Result<(Picked<'f>, Vec<FieldError>), ScanError> {
        let (value, notes) = resource_network_global::read(
            tree,
            &self.projection,
            admission,
            self.scan.options.cancel,
        )?;
        Ok((Picked::ResourceNetworkGlobal(Box::new(value)), notes))
    }

    pub(super) fn resource_group(
        &self,
        admission: &RecordObservation<'_, 'f>,
        tree: &RecordTree<'f>,
    ) -> Result<(Picked<'f>, Vec<FieldError>), ScanError> {
        let parent = self
            .scan
            .groups
            .parent_choice(&tree.guid(), self.scan.options.cancel)?;
        let read = crate::resource_group_read::read(
            tree,
            &self.projection,
            parent,
            admission,
            self.scan.options.cancel,
        )?;
        Ok((Picked::ResourceGroup(Box::new(read.value)), read.notes))
    }

    pub(super) fn commodity(
        &self,
        trees: &mut TreeBuilder<'f>,
        admission: &RecordObservation<'_, 'f>,
        tree: &RecordTree<'f>,
        ledger: &mut ExportLedger,
    ) -> Result<Picked<'f>, ScanError> {
        let selected = commodity_resource::first(tree)?;
        let resource_type = selected
            .link
            .as_ref()
            .map(|link| self.resource_type(trees, admission, tree, link, ledger))
            .transpose()?
            .flatten();
        let read = commodity::commodity(
            tree,
            &self.projection,
            resource_type.as_ref().map(|resource| &resource.tree),
            &self.scan.groups,
            &selected,
        )?;
        crate::source_projection_notes::record(
            Kind::Commodities,
            ExportSourceRole::Selected,
            admission,
            tree,
            read.notes,
            ledger,
        )?;
        if let Some(resource) = &resource_type {
            crate::source_projection_notes::record(
                Kind::Commodities,
                ExportSourceRole::Dependency,
                &RecordObservation::capture(self.projection.original(), &resource.record)?,
                &resource.tree,
                read.resource_notes,
                ledger,
            )?;
        }
        Ok(Picked::Commodity(Box::new(read.value)))
    }
    fn resource_type(
        &self,
        trees: &mut TreeBuilder<'f>,
        commodity: &RecordObservation<'_, 'f>,
        commodity_tree: &RecordTree<'f>,
        link: &CommodityResource<'_, 'f>,
        ledger: &mut ExportLedger,
    ) -> Result<Option<ResourceRead<'f>>, ScanError> {
        if link.reference.target_type.as_deref() != Some("ResourceType") {
            return Ok(None);
        }
        let Some(index) = link.reference.target_record_index else {
            return Ok(None);
        };
        let record = self
            .scan
            .forge
            .record(index)
            .ok_or(CatalogError::Accounting(
                ExportAccountingError::MissingPhysicalReferenceTarget { index },
            ))?;
        match trees.build(&record) {
            Ok(tree) => {
                self.projection.original().record_scope(&record, &tree)?;
                Ok(Some(ResourceRead { record, tree }))
            }
            Err(error) => {
                let failure = crate::export_source_failure::diagnosis(&error);
                let body = link.failure_body(
                    self.projection.original(),
                    commodity,
                    commodity_tree,
                    self.scan.options.cancel,
                )?;
                let diagnosis = SourceDiagnosis::from_body(
                    failure
                        .code
                        .unwrap_or(ExportDiagnosticCode::SourceReadFailure),
                    body,
                )?;
                ledger.derived_failure(
                    DerivedFailure::of_subject(
                        commodity.subject().clone(),
                        Kind::Commodities,
                        Kind::Commodities,
                        ExportSourceRole::Selected,
                        diagnosis,
                    )
                    .map_err(CatalogError::from)?,
                );
                Ok(None)
            }
        }
    }
}

impl<'f> Scan<'_, '_, 'f> {
    /// Reads the used additional resource definitions at their actual projection boundary.
    pub(super) fn resource_commodities(
        &self,
        presence: CommodityPresence,
        collected: &Collected<'f>,
        scanned: &ExportLedger,
    ) -> Result<ResourceRows<'f>, CatalogError> {
        let used = unrepresented_resources(
            &presence,
            &collected.blueprints,
            collected.quality.fallbacks.targets(),
        );
        let physical: BTreeSet<_> = scanned
            .sources
            .iter()
            .filter(|source| {
                source.kind == Some(Kind::ResourceTypes)
                    && used.contains(&source.guid)
                    && (source.role == ExportSourceRole::SelectionCandidate
                        || (source.role == ExportSourceRole::Selected
                            && !matches!(
                                source.disposition,
                                SourceDisposition::SkippedBeforeRead
                                    | SourceDisposition::SkippedAfterRead
                            )))
            })
            .map(|source| source.record_index)
            .collect();
        if physical.is_empty() {
            return Ok(ResourceRows::default());
        }
        let context = BackfillContext {
            representation: presence.into_origin(scanned, self.projection.origins()),
            collected,
        };
        let mut trees = TreeBuilder::new();
        let mut read = ResourceRows::default();
        for index in physical {
            if self.options.cancel.is_some_and(CancelToken::is_cancelled) {
                return Err(CatalogError::Cancelled);
            }
            let record = self.forge.record(index).ok_or(CatalogError::Accounting(
                ExportAccountingError::MissingPhysicalReferenceTarget { index },
            ))?;
            let mut trace = SourceTrace::new(&record);
            trace.candidate(Kind::Commodities);
            let mut ledger = ExportLedger::default();
            let result = self.resource(&mut trees, &record, &mut trace, &mut ledger, &context);
            if let Some(found) = crate::scan_health::finish(
                self.projection.original(),
                &record,
                &trace,
                result,
                ledger,
            )? {
                read.picked.extend(found.picked);
                read.entity_tags.extend(found.entity.tags);
                read.ledger.merge(found.ledger);
            }
        }
        Ok(read)
    }

    fn resource(
        &self,
        trees: &mut TreeBuilder<'f>,
        record: &Record<'f>,
        trace: &mut SourceTrace,
        ledger: &mut ExportLedger,
        context: &BackfillContext<'_, 'f>,
    ) -> Result<Option<Picked<'f>>, ScanError> {
        if record.type_name()? != RESOURCE_TYPE {
            return Ok(None);
        }
        let path = record.record_path()?;
        if self
            .options
            .filter
            .is_some_and(|filter| !filter.matches(path))
        {
            return Ok(None);
        }
        if crate::selection::is_kind_leftover(Kind::ResourceTypes, path, record.name()?) {
            return Ok(None);
        }
        trace.selected(Kind::Commodities);
        let admission = RecordObservation::capture(self.projection.original(), record)?;
        let tree = trees.build(record)?;
        trace.projecting();
        ledger.merge(crate::export_field_walk::inspect_observed(
            Some(self.projection.original()),
            &tree,
            &admission,
            Some(Kind::Commodities),
            ExportSourceRole::Selected,
            self.options.cancel,
        )?);
        let selection = commodity_backfill::for_resource(
            &tree,
            &context.representation,
            &context.collected.blueprints,
            &context.collected.quality,
            self.projection.origins(),
        )?;
        let projection = self.projection.for_kind(Kind::Commodities);
        let read = commodity::resource_commodity(&tree, &projection, &self.groups, selection)?;
        crate::source_projection_notes::record(
            Kind::Commodities,
            ExportSourceRole::Selected,
            &admission,
            &tree,
            read.resource_notes,
            ledger,
        )?;
        Ok(Some(Picked::Commodity(Box::new(read.value))))
    }
}

fn unrepresented_resources(
    presence: &CommodityPresence,
    blueprints: &[BlueprintRow<'_>],
    quantizations: impl Iterator<Item = Guid>,
) -> HashSet<Guid> {
    blueprints
        .iter()
        .flat_map(|blueprint| blueprint.resources.targets())
        .chain(quantizations)
        .filter(|guid| !presence.represents(guid))
        .collect()
}
