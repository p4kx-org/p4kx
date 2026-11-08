// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use p4kx_assembly::{ArchiveFiles, Entities};
use p4kx_extraction::{
    CancelToken, DataForge, DataForgeError, Filter, Guid, PointerProbe, Record, TreeBuilder,
};
use rayon::prelude::*;

use crate::catalog_crafting::CraftingKinds;
use crate::catalog_projection::CatalogProjection;
use crate::contract_recipe::CostLine;
use crate::entity_selection::{Selected, select};
use crate::export_diagnostic::ExportSourceRole;
use crate::export_ledger::{ExportLedger, SourceDisposition, SourceObservation};
use crate::harvest_link::Links;
use crate::item::is_paint;
use crate::picked::Picked;
use crate::playability_rules::Evidence;
use crate::resource_group::Groups;
use crate::scan_health::{RecordScan, ScanError, ScanRead, SourceTrace};
use crate::scan_signature::SignatureSlot;
use crate::ship_pool::ShipPool;
use crate::starmap::TypeNames;
use crate::starmap_containers::Placements;
use crate::texts::Texts;
use crate::{CatalogError, CatalogOptions, CatalogPhase, CatalogProgress, Kind};

const PROGRESS_STEP: usize = 1024;
const RESOURCE_TYPE: &str = "ResourceType";
const CRAFTER_FIELD: &str = "Components";
const CRAFTER_STRUCT: &str = "CrafterComponentParams";

#[path = "scan_groups.rs"]
mod groups;
#[path = "scan_projection.rs"]
mod projection;
#[path = "scan_resources.rs"]
mod resources;

/// Returns whether the `resourceGroups` kind selects `record` under `filter`; a Record whose path
/// or type cannot be read is not selected here, the check of the Record reports it.
fn selects_group(record: &Record<'_>, filter: Option<&Filter>) -> bool {
    matches!(
        select(record, filter),
        Ok(Some(Selected::Entity(Kind::ResourceGroups)))
    )
}

/// Appends to `out` the `ResourceType` GUID of every resource line of `line`, selections opened.
fn resources_of(line: &CostLine<'_>, out: &mut Vec<Guid>) {
    out.extend(line.resource_guid.map(|guid| guid.0));
    for option in &line.options {
        resources_of(option, out);
    }
}

/// The parallel check of every Record, with its progress.
pub(crate) struct Scan<'o, 'a, 'f> {
    forge: &'f DataForge<'f>,
    ships: ShipPool<'f>,
    options: &'o CatalogOptions<'a>,
    texts: &'o Texts<'f>,
    projection: &'o CatalogProjection<'o, 'f>,
    placements: &'o Placements,
    types: TypeNames<'f>,
    links: Links<'f>,
    crafters: Option<PointerProbe<'f>>,
    groups: Groups,
    evidence: Evidence,
    signature_slot: SignatureSlot,
    done: AtomicUsize,
    total: usize,
}

impl<'o, 'a, 'f> Scan<'o, 'a, 'f> {
    /// Checks every Record of `forge` in parallel and returns those a kind selected; a ship's base
    /// data is read here, once, by the worker that checks it.
    pub(crate) fn run(
        forge: &'f DataForge<'f>,
        files: &'f ArchiveFiles<'f>,
        entities: Arc<Entities<'f>>,
        options: &'o CatalogOptions<'a>,
        projection: &'o CatalogProjection<'o, 'f>,
        crafting: &CraftingKinds<'f>,
        placements: &'o Placements,
    ) -> Result<ScanRead<'f>, CatalogError> {
        let records: Vec<Record<'f>> = forge.records().collect();
        let groups::GroupRead {
            groups,
            failures: group_failures,
            ledger: group_ledger,
        } = Self::groups(&records, options.filter, options.cancel)?;
        let (evidence, evidence_failures, evidence_ledger) =
            Evidence::read_catalog(&records, options.cancel)?;
        let (signature_slot, slot_failures, slot_problem, slot_ledger) =
            SignatureSlot::read_catalog(&records, options.cancel)?;
        let scan = Self {
            forge,
            ships: ShipPool::new(entities, files),
            options,
            texts: projection.legacy_texts(),
            projection,
            placements,
            types: TypeNames::default(),
            links: Links::default(),
            crafters: forge.pointer_probe(CRAFTER_FIELD, CRAFTER_STRUCT),
            groups,
            evidence,
            signature_slot,
            done: AtomicUsize::new(0),
            total: records.len(),
        };
        let checked: Vec<RecordScan<'f>> = records
            .par_iter()
            .map_init(TreeBuilder::new, |trees, record| scan.check(trees, record))
            .filter_map(Result::transpose)
            .collect::<Result<_, _>>()?;
        let mut ledger = group_ledger;
        ledger.merge(evidence_ledger);
        ledger.merge(slot_ledger);
        scan.complete(
            checked,
            ledger,
            group_failures
                .into_iter()
                .chain(evidence_failures)
                .chain(slot_failures),
            slot_problem,
            crafting,
        )
    }

    fn complete(
        &self,
        checked: Vec<RecordScan<'f>>,
        mut ledger: ExportLedger,
        failures: impl IntoIterator<Item = crate::CatalogFailure>,
        slot_problem: Option<crate::scan_signature::SlotProblem>,
        crafting: &CraftingKinds<'f>,
    ) -> Result<ScanRead<'f>, CatalogError> {
        let mut picked = Vec::new();
        for checked in checked {
            picked.extend(checked.picked);
            ledger.merge(checked.ledger);
        }
        picked.extend(failures.into_iter().map(Picked::Failed));
        let resources = self.resource_commodities(
            &picked,
            &crafting.blueprints,
            &crafting.quantizations,
            &ledger.sources,
        )?;
        picked.extend(resources.picked);
        ledger.merge(resources.ledger);
        picked.extend(self.links.warning().map(Picked::Warned));
        let has_signatures = picked.iter().any(
            |pick| matches!(pick, Picked::MineableEntity(entity) if !entity.signatures.is_empty()),
        );
        if self.options.filter.is_none() && has_signatures {
            picked.extend(slot_problem.map(|problem| Picked::Warned(problem.warning())));
        }
        let inventories = Kind::ALL
            .into_iter()
            .filter(|kind| !kind.is_crafting() && !kind.is_derived());
        for kind in inventories {
            ledger.inventory(kind);
        }
        Ok(ScanRead { picked, ledger })
    }

    fn check(
        &self,
        trees: &mut TreeBuilder<'f>,
        record: &Record<'f>,
    ) -> Result<Option<RecordScan<'f>>, CatalogError> {
        if self.options.cancel.is_some_and(CancelToken::is_cancelled) {
            return Err(CatalogError::Cancelled);
        }
        self.tick();
        let mut trace = SourceTrace::new(record);
        let mut ledger = ExportLedger::default();
        let capture = Arc::new(crate::export_builder_capture::BuilderCapture::for_root(
            record.index(),
            self.options.cancel,
        ));
        trees.set_read_observer(Some(capture.clone()));
        let result = self.select(trees, record, &mut trace, &mut ledger, &capture);
        trees.set_read_observer(None);
        ledger.merge(capture.finish()?);
        crate::scan_health::finish(record, &trace, result, ledger)
    }

    /// Returns what a development leftover of kind counts as: a skipped Record, a skipped paint
    /// for an item that is one, and for a crafter only a Record that holds the component.
    fn leftover(
        &self,
        trees: &mut TreeBuilder<'f>,
        record: &Record<'f>,
        kind: Kind,
        ledger: &mut ExportLedger,
    ) -> Result<Option<Picked<'f>>, ScanError> {
        if kind == Kind::Items {
            let tree = match trees.build(record) {
                Ok(tree) => tree,
                Err(error) => {
                    ledger.observe(
                        SourceObservation::new(
                            kind,
                            record,
                            ExportSourceRole::Dependency,
                            SourceDisposition::ReadFailed,
                        )
                        .diagnosed(crate::export_source_failure::diagnosis(&error)),
                    );
                    return Ok(Some(Picked::Skipped(kind, record.guid())));
                }
            };
            ledger.merge(crate::export_field_walk::inspect(
                &tree,
                record,
                Some(kind),
                ExportSourceRole::Dependency,
                self.options.cancel,
            )?);
            ledger.observe(SourceObservation::new(
                kind,
                record,
                ExportSourceRole::Dependency,
                SourceDisposition::Projected,
            ));
            return Ok(Some(if is_paint(&tree) {
                Picked::SkippedPaint
            } else {
                Picked::Skipped(kind, record.guid())
            }));
        }
        Ok(Some(Picked::Skipped(kind, record.guid())))
    }

    fn select(
        &self,
        trees: &mut TreeBuilder<'f>,
        record: &Record<'f>,
        trace: &mut SourceTrace,
        ledger: &mut ExportLedger,
        capture: &crate::export_builder_capture::BuilderCapture,
    ) -> Result<Option<Picked<'f>>, ScanError> {
        let kind = match select(record, self.options.filter)? {
            None => return Ok(None),
            Some(Selected::Leftover(kind)) => {
                if kind == Kind::Crafters {
                    trace.candidate(kind);
                    if !self.holds_crafter(trees, record)? {
                        return Ok(None);
                    }
                }
                trace.selected(kind);
                capture.activate(kind)?;
                return self.leftover(trees, record, kind, ledger);
            }
            Some(Selected::Entity(Kind::Crafters)) => {
                trace.candidate(Kind::Crafters);
                if !self.holds_crafter(trees, record)? {
                    return Ok(None);
                }
                Kind::Crafters
            }
            Some(Selected::Entity(kind)) => kind,
        };
        trace.selected(kind);
        capture.activate(kind)?;
        let tree = trees.build(record)?;
        trace.projecting();
        ledger.merge(crate::export_field_walk::inspect(
            &tree,
            record,
            Some(kind),
            ExportSourceRole::Selected,
            self.options.cancel,
        )?);
        self.project(kind, trees, record, &tree, ledger)
    }

    /// Returns whether the Components of `record` hold a crafter component, read without building
    /// the Record tree; alse without any such component in the DataForge.
    fn holds_crafter(
        &self,
        trees: &mut TreeBuilder<'f>,
        record: &Record<'f>,
    ) -> Result<bool, DataForgeError> {
        self.crafters
            .as_ref()
            .map_or(Ok(false), |probe| trees.holds(probe, record))
    }

    fn tick(&self) {
        let done = self.done.fetch_add(1, Ordering::Relaxed).saturating_add(1);
        if let Some(progress) = self.options.progress
            && (done.is_multiple_of(PROGRESS_STEP) || done == self.total)
        {
            progress(CatalogProgress {
                phase: CatalogPhase::Records,
                records_done: done,
                records_total: self.total,
            });
        }
    }
}

#[cfg(test)]
mod tests;
