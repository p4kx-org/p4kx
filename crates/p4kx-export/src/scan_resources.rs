// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeSet, HashSet};

use p4kx_extraction::{CancelToken, Guid, Record, TreeBuilder};

use super::{RESOURCE_TYPE, Scan};
use crate::blueprint_catalog_rows::BlueprintRow;
use crate::catalog_crafting::CraftingKinds;
use crate::commodity;
use crate::export_diagnostic::ExportSourceRole;
use crate::export_ledger::{ExportLedger, SourceDisposition};
use crate::picked::Picked;
use crate::record_observation::RecordObservation;
use crate::scan_health::{ScanError, ScanRead, SourceTrace};
use crate::{CatalogError, Kind};

struct BackfillContext<'context, 'source> {
    picked: &'context [Picked<'source>],
    crafting: &'context CraftingKinds<'source>,
    scanned: &'context ExportLedger,
}
impl<'f> Scan<'_, '_, 'f> {
    /// Reads the used additional resource definitions at their actual projection boundary.
    pub(super) fn resource_commodities(
        &self,
        picked: &[Picked<'f>],
        crafting: &CraftingKinds<'f>,
        scanned: &ExportLedger,
    ) -> Result<ScanRead<'f>, CatalogError> {
        let used = unrepresented_resources(
            picked,
            &crafting.blueprints,
            crafting.quality.fallbacks.targets(),
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
        let context = BackfillContext {
            picked,
            crafting,
            scanned,
        };
        let mut trees = TreeBuilder::new();
        let mut read = ScanRead {
            picked: Vec::new(),
            entity_tags: Vec::new(),
            resource_group_members: Vec::new(),
            ledger: ExportLedger::default(),
        };
        for index in physical {
            if self.options.cancel.is_some_and(CancelToken::is_cancelled) {
                return Err(CatalogError::Cancelled);
            }
            let record = self.forge.record(index).ok_or(CatalogError::Accounting(
                crate::ExportAccountingError::MissingPhysicalReferenceTarget { index },
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
                read.entity_tags.extend(found.entity_tags);
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
        let evidence = crate::commodity_backfill::for_resource(
            &tree,
            context.picked,
            context.crafting,
            context.scanned,
            self.projection.origins(),
        )?;
        let read = commodity::resource_commodity(&tree, self.projection, &self.groups, &evidence)?;
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
    picked: &[Picked<'_>],
    blueprints: &[BlueprintRow<'_>],
    quantizations: impl Iterator<Item = Guid>,
) -> HashSet<Guid> {
    let mut carried = HashSet::new();
    for pick in picked {
        if let Picked::Commodity(commodity) = pick {
            carried.extend(commodity.subtype_guid.map(|guid| guid.0));
        }
    }
    blueprints
        .iter()
        .flat_map(|blueprint| blueprint.resources.targets())
        .chain(quantizations)
        .filter(|guid| !carried.contains(guid))
        .collect()
}
