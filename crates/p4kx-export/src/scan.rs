// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use p4kx_assembly::{ArchiveFiles, Entities};
use p4kx_extraction::{
    CancelToken, DataForge, DataForgeError, Filter, PointerProbe, Record, TreeBuilder,
};

use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_crafting::CraftingKinds;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_v3::catalog_entity_configurations;
use crate::catalog_v3::catalog_item_alignment::AlignmentOptions;
use crate::collected::Collected;
use crate::entity_selection::{Selected, select};
use crate::export_diagnostic::ExportSourceRole;
use crate::export_ledger::{ExportLedger, SourceDisposition};
use crate::harvest_link::Links;
use crate::item::is_paint;
use crate::picked::Picked;
use crate::playability_rules::Evidence;
use crate::record_observation::RecordObservation;
use crate::resource_group::Groups;
use crate::scan_health::{RecordEntityRows, RecordScan, ScanError, SourceTrace};
use crate::scan_signature::SignatureSlot;
use crate::ship_pool::ShipPool;
use crate::starmap::TypeNames;
use crate::texts::Texts;
use crate::{CatalogError, CatalogOptions, CatalogPhase, CatalogProgress, Kind};

const PROGRESS_STEP: usize = 1024;
const RESOURCE_TYPE: &str = "ResourceType";
const CRAFTER_FIELD: &str = "Components";
const CRAFTER_STRUCT: &str = "CrafterComponentParams";

#[path = "scan_ammo.rs"]
mod ammo;
#[path = "scan_complete.rs"]
mod complete;
#[path = "scan_definition_projection.rs"]
mod definition_projection;
#[path = "scan_domain.rs"]
mod domain;
pub(crate) use domain::ScanDomain;
#[path = "scan_groups.rs"]
mod groups;
#[path = "scan_projection.rs"]
mod projection;
use self::projection::SelectedProjection;
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

/// Checks all eligible Records and reports scan progress.
pub(crate) struct Scan<'o, 'a, 'f> {
    forge: &'f DataForge<'f>,
    original: &'f OriginalCatalogInput<'f, 'f>,
    ships: ShipPool<'f>,
    ammo: ammo::AmmoReads<'a, 'o, 'f>,
    options: &'o CatalogOptions<'a>,
    texts: &'o Texts<'f>,
    projection: &'o CatalogProjection<'o, 'f>,
    domain: ScanDomain<'o>,
    types: TypeNames<'f>,
    links: Links<'f>,
    crafters: Option<PointerProbe<'f>>,
    groups: Groups,
    evidence: Evidence,
    signature_slot: SignatureSlot,
    alignment_options: AlignmentOptions<'f>,
    done: AtomicUsize,
    total: usize,
}

struct ScanCandidate<'record, 'f> {
    record: &'record Record<'f>,
    selection: Result<Option<Selected>, DataForgeError>,
}

impl<'o, 'a, 'f> Scan<'o, 'a, 'f> {
    /// Projects selected Records and reads each ship base in physical Record order.
    pub(crate) fn run(
        original: &'f OriginalCatalogInput<'f, 'f>,
        files: &'f ArchiveFiles<'f>,
        entities: Arc<Entities<'f>>,
        options: &'o CatalogOptions<'a>,
        projection: &'o CatalogProjection<'o, 'f>,
        crafting: CraftingKinds<'f>,
        domain: ScanDomain<'o>,
    ) -> Result<Collected<'f>, CatalogError> {
        let forge = original.forge();
        projection.original().check_forge(forge)?;
        let records = domain.records(forge);
        let groups::GroupRead {
            groups,
            members,
            failures: group_failures,
            ledger: group_ledger,
        } = Self::groups(
            &records,
            options.filter,
            projection.original(),
            options.cancel,
        )?;
        let (evidence, evidence_failures, evidence_ledger) =
            Evidence::read_captured(&records, projection.original(), options.cancel)?;
        let (signature_slot, slot_failures, slot_problem, slot_ledger) =
            SignatureSlot::read_captured(&records, projection.original(), options.cancel)?;
        let scan = Self {
            forge,
            original,
            ships: ShipPool::new(entities, files),
            ammo: ammo::AmmoReads::new(projection, options.filter, options.cancel),
            options,
            texts: projection.legacy_texts(),
            projection,
            domain,
            types: TypeNames::default(),
            links: Links::default(),
            crafters: forge.pointer_probe(CRAFTER_FIELD, CRAFTER_STRUCT),
            groups,
            evidence,
            signature_slot,
            alignment_options: AlignmentOptions::read(forge, options.cancel)?,
            done: AtomicUsize::new(0),
            total: records.len(),
        };
        let mut ledger = group_ledger;
        ledger.merge(evidence_ledger);
        ledger.merge(slot_ledger);
        let mut read = scan.complete(
            &records,
            ledger,
            group_failures
                .into_iter()
                .chain(evidence_failures)
                .chain(slot_failures),
            slot_problem,
            crafting,
        )?;
        read.resource_group_members = members;
        Ok(read)
    }

    fn check(
        &self,
        trees: &mut TreeBuilder<'f>,
        candidate: ScanCandidate<'_, 'f>,
    ) -> Result<Option<RecordScan<'f>>, CatalogError> {
        if self.options.cancel.is_some_and(CancelToken::is_cancelled) {
            return Err(CatalogError::Cancelled);
        }
        self.tick();
        let record = candidate.record;
        let mut trace = SourceTrace::new(record);
        let mut ledger = ExportLedger::default();
        let capture = Arc::new(
            crate::export_builder_capture::BuilderCapture::for_input_root(
                self.original,
                record,
                self.options.cancel,
            )?,
        );
        trees.set_read_observer(Some(capture.clone()));
        let mut entity = RecordEntityRows::default();
        let result = self.select(
            trees,
            candidate,
            &mut trace,
            &mut ledger,
            &capture,
            &mut entity,
        );
        trees.set_read_observer(None);
        ledger.merge(capture.finish()?);
        let mut checked =
            crate::scan_health::finish(self.original, record, &trace, result, ledger)?;
        if let Some(checked) = &mut checked {
            checked.entity = entity;
        }
        Ok(checked)
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
            let admission = RecordObservation::capture(self.original, record)?;
            let tree = match trees.build(record) {
                Ok(tree) => tree,
                Err(error) => {
                    ledger.observe(
                        admission
                            .source(
                                Some(kind),
                                ExportSourceRole::Dependency,
                                SourceDisposition::ReadFailed,
                            )?
                            .diagnosed(crate::export_source_failure::diagnosis(&error)),
                    );
                    return Ok(Some(Picked::Skipped(kind, record.guid())));
                }
            };
            ledger.merge(crate::export_field_walk::inspect_observed(
                Some(self.original),
                &tree,
                &admission,
                Some(kind),
                ExportSourceRole::Dependency,
                self.options.cancel,
            )?);
            ledger.observe(admission.source(
                Some(kind),
                ExportSourceRole::Dependency,
                SourceDisposition::Projected,
            )?);
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
        candidate: ScanCandidate<'_, 'f>,
        trace: &mut SourceTrace,
        ledger: &mut ExportLedger,
        capture: &crate::export_builder_capture::BuilderCapture<'_>,
        entity: &mut RecordEntityRows<'f>,
    ) -> Result<Option<Picked<'f>>, ScanError> {
        let ScanCandidate { record, selection } = candidate;
        let kind = match selection? {
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
        self.project_selected(kind, trees, record, trace, ledger, entity)
    }

    fn project_selected(
        &self,
        kind: Kind,
        trees: &mut TreeBuilder<'f>,
        record: &Record<'f>,
        trace: &mut SourceTrace,
        ledger: &mut ExportLedger,
        entity: &mut RecordEntityRows<'f>,
    ) -> Result<Option<Picked<'f>>, ScanError> {
        if kind == Kind::AmmoDefinitions {
            let definition = self.ammo.selected(record, ledger)?;
            trace.projecting();
            return Ok(Some(Picked::AmmoDefinition(Box::new(definition))));
        }
        let admission = RecordObservation::capture(self.original, record)?;
        let tree = trees.build(record)?;
        trace.projecting();
        ledger.merge(crate::export_field_walk::inspect_observed(
            Some(self.original),
            &tree,
            &admission,
            Some(kind),
            ExportSourceRole::Selected,
            self.options.cancel,
        )?);
        let selected = SelectedProjection::new(self, kind);
        if tree.type_name() == "EntityClassDefinition" {
            let input = self.original;
            let cancel = self.options.cancel;
            let (tags, mut notes) =
                crate::catalog_v3::tag_catalog::entity(&tree, input, &admission, cancel)?;
            let (configurations, configuration_notes) = catalog_entity_configurations::read(
                &tree,
                &selected.projection,
                &admission,
                &self.alignment_options,
                cancel,
            )?;
            notes.extend(configuration_notes);
            crate::source_projection_notes::record(
                kind,
                ExportSourceRole::Selected,
                &admission,
                &tree,
                notes,
                ledger,
            )?;
            entity.tags = Some(tags);
            entity.configurations = configurations;
        }
        selected.project(trees, &admission, &tree, ledger)
    }

    /// Returns whether the Components of `record` hold a crafter component, read without building
    /// the Record tree; `false` without any such component in the DataForge.
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
