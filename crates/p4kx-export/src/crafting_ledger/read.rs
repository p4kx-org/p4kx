// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge, Filter, Record, RecordTree, TreeBuilder};
use rayon::prelude::*;

use crate::blueprint_rule_inputs::BlueprintRules;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_read_failure::CatalogReadFailure;
use crate::crafting_scan;
use crate::crafting_warning::CraftingWarning;
use crate::export_diagnostic::ExportSourceRole;
use crate::export_ledger::{ExportLedger, SourceDisposition};
use crate::record_observation::RecordObservation;
use crate::{CatalogError, CatalogProgress, Kind};

use super::fault::Fault;
use super::projection::{self, Piece, ProjectError};
use super::resource_scope::ResourceLinkScope;
use super::selection::{self, Action, ReadPolicy, Selection};
use super::{ReadCrafting, ReadMode};
use crate::catalog_v3::gameplay_property_catalog::GameplayPropertyInput;
use crate::quality_catalog::Step;

pub(super) struct Tracked<'f> {
    pub(super) piece: Option<Piece<'f>>,
    pub(super) blueprint_rules: BlueprintRules,
    pub(super) ledger: ExportLedger,
    pub(super) quality: Step<'f>,
    pub(super) gameplay: Option<GameplayPropertyInput<'f>>,
}

fn candidate<'f>(
    admission: &RecordObservation<'_, 'f>,
    error: &p4kx_extraction::DataForgeError,
    filter: Option<&Filter>,
) -> Result<Tracked<'f>, CatalogError> {
    let record = admission.record();
    let source = admission.source(
        selection::candidate_kind(record),
        ExportSourceRole::SelectionCandidate,
        SourceDisposition::ReadFailed,
    )?;
    let mut ledger = ExportLedger::default();
    ledger.observe(source);
    Ok(Tracked {
        piece: (!selection::outside(record, filter))
            .then(|| Piece::Failed(CatalogReadFailure::of(record, error))),
        ledger,
        quality: Step::default(),
        gameplay: None,
        blueprint_rules: BlueprintRules::default(),
    })
}
fn failure<'f>(
    selected: Selection,
    admission: &RecordObservation<'_, 'f>,
    fault: Fault<'_>,
    mut ledger: ExportLedger,
    filter: Option<&Filter>,
) -> Result<Tracked<'f>, CatalogError> {
    let record = admission.record();
    let mut source = selected
        .observation(admission, fault.disposition())?
        .diagnosed(fault.diagnosis());
    if selected.policy == ReadPolicy::SkipAfter {
        ledger.observe(selected.observation(admission, SourceDisposition::SkippedBeforeRead)?);
        source.role = ExportSourceRole::Dependency;
        ledger.reassign_inspection_role(
            selected.action.kind(),
            record.index(),
            selected.role,
            ExportSourceRole::Dependency,
        );
    }
    ledger.observe(source);
    Ok(Tracked {
        piece: fault
            .compatibility_failure(record, filter)
            .then(|| Piece::Failed(CatalogReadFailure::of(record, fault.error()))),
        ledger,
        quality: Step::default(),
        gameplay: None,
        blueprint_rules: BlueprintRules::default(),
    })
}

fn one<'f>(
    trees: &mut TreeBuilder<'f>,
    record: &Record<'f>,
    filter: Option<&Filter>,
    cancel: Option<&CancelToken>,
    scope: ResourceLinkScope,
    mode: ReadMode<'_, 'f>,
) -> Result<Option<Tracked<'f>>, CatalogError> {
    if !scope.includes(record, filter) {
        return Ok(None);
    }
    let (selected, inventory) = mode.select(record, filter)?;
    let mut tracked = match selected {
        Ok(Some(selected)) => selected_record(trees, record, filter, cancel, mode, selected)?,
        Ok(None) if mode.origins().is_some() => Tracked {
            piece: None,
            ledger: ExportLedger::default(),
            quality: Step::default(),
            gameplay: None,
            blueprint_rules: BlueprintRules::default(),
        },
        Ok(None) => return Ok(None),
        Err(error) => {
            let admission = mode.admission(record)?;
            let mut tracked = candidate(&admission, &error, filter)?;
            tracked.quality = mode.failed(record, None)?;
            tracked
        }
    };
    tracked.quality.inventory.append(&inventory)?;
    Ok(Some(tracked))
}

fn selected_record<'f>(
    trees: &mut TreeBuilder<'f>,
    record: &Record<'f>,
    filter: Option<&Filter>,
    cancel: Option<&CancelToken>,
    mode: ReadMode<'_, 'f>,
    selected: Selection,
) -> Result<Tracked<'f>, CatalogError> {
    let admission = mode.admission(record)?;
    if selected.policy == ReadPolicy::SkipBefore {
        let mut ledger = ExportLedger::default();
        ledger.observe(selected.observation(&admission, SourceDisposition::SkippedBeforeRead)?);
        return Ok(Tracked {
            piece: Some(Piece::Skipped),
            ledger,
            quality: Step::default(),
            gameplay: None,
            blueprint_rules: BlueprintRules::default(),
        });
    }
    let tree = match trees.build(record) {
        Ok(tree) => tree,
        Err(error) => {
            let mut tracked = failure(
                selected,
                &admission,
                Fault::Read(&error),
                ExportLedger::default(),
                filter,
            )?;
            tracked.quality = mode.failed(record, Some(selected.action))?;
            if mode.input().is_some() {
                match selected.action {
                    Action::Globals => {
                        tracked.blueprint_rules.defaults.unavailable = true;
                        tracked.blueprint_rules.item_exclusions.unavailable = true;
                    }
                    Action::Dismantle => tracked.blueprint_rules.process_unavailable = true,
                    _ => {}
                }
            }
            return Ok(tracked);
        }
    };
    project_tree(&admission, &tree, selected, mode, cancel, filter)
}

fn read_blueprint<'source>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    role: ExportSourceRole,
    ledger: &mut ExportLedger,
    cancel: Option<&CancelToken>,
) -> Result<Piece<'source>, CatalogError> {
    let found = crate::blueprint_catalog_read::read(tree, projection, admission, cancel)?;
    crate::source_projection_notes::record(
        Kind::Blueprints,
        role,
        admission,
        tree,
        found.notes,
        ledger,
    )?;
    Ok(Piece::CanonicalBlueprint(Box::new(found.value)))
}

fn read_selected_projection<'source>(
    selected: Selection,
    admission: &RecordObservation<'_, 'source>,
    tree: &RecordTree<'source>,
    ledger: &mut ExportLedger,
    cancel: Option<&CancelToken>,
) -> Result<Option<Piece<'source>>, ProjectError> {
    let projected = projection::project(selected.action, tree, cancel)?;
    if let Some(kind) = selected.action.kind() {
        crate::source_projection_notes::record(
            kind,
            selected.role,
            admission,
            tree,
            projected.notes,
            ledger,
        )
        .map_err(ProjectError::Catalog)?;
    }
    Ok(projected.piece)
}

fn project_tree<'f>(
    admission: &RecordObservation<'_, 'f>,
    tree: &RecordTree<'f>,
    selected: Selection,
    mode: ReadMode<'_, 'f>,
    cancel: Option<&CancelToken>,
    filter: Option<&Filter>,
) -> Result<Tracked<'f>, CatalogError> {
    let mut ledger = crate::export_field_walk::inspect_observed(
        mode.input(),
        tree,
        admission,
        selected.action.kind(),
        selected.role,
        cancel,
    )?;
    let input = mode.project(selected, admission, tree, &mut ledger, cancel)?;
    let gameplay = mode.property(selected, admission, tree, &mut ledger, cancel)?;
    let piece = if input.is_some() || gameplay.is_some() {
        None
    } else if let ReadMode::Catalog(projection) = mode
        && selected.action == Action::Blueprint
    {
        Some(read_blueprint(
            tree,
            projection,
            admission,
            selected.role,
            &mut ledger,
            cancel,
        )?)
    } else {
        match read_selected_projection(selected, admission, tree, &mut ledger, cancel) {
            Ok(piece) => piece,
            Err(ProjectError::Catalog(error)) => return Err(error),
            Err(ProjectError::Source(error)) => {
                return failure(
                    selected,
                    admission,
                    Fault::Projection(&error),
                    ledger,
                    filter,
                );
            }
        }
    };
    let blueprint_rules = mode.blueprint_rules(selected.action, admission, tree, cancel)?;
    ledger.observe(selected.observation(admission, selected.successful_disposition())?);
    Ok(Tracked {
        piece,
        ledger,
        gameplay,
        blueprint_rules,
        quality: Step {
            input,
            ..Step::default()
        },
    })
}

fn inventories(ledger: &mut ExportLedger) {
    for kind in [
        Kind::Blueprints,
        Kind::GameplayProperties,
        Kind::QualityDistributions,
        Kind::QualityQuantizations,
        Kind::BlueprintPools,
        Kind::DismantleProcesses,
    ] {
        ledger.inventory(kind);
    }
}

pub(crate) fn read<'f>(
    forge: &'f DataForge<'f>,
    filter: Option<&Filter>,
    cancel: Option<&CancelToken>,
    progress: Option<&(dyn Fn(CatalogProgress) + Sync)>,
) -> Result<ReadCrafting<'f>, CatalogError> {
    let records: Vec<_> = forge.records().collect();
    read_scoped(
        &records,
        filter,
        cancel,
        progress,
        ResourceLinkScope::AllResources,
        ReadMode::Interpreted,
    )
}

pub(crate) fn read_catalog<'f>(
    forge: &'f DataForge<'f>,
    filter: Option<&Filter>,
    cancel: Option<&CancelToken>,
    progress: Option<&(dyn Fn(CatalogProgress) + Sync)>,
    mode: ReadMode<'_, 'f>,
) -> Result<ReadCrafting<'f>, CatalogError> {
    let records: Vec<_> = forge.records().collect();
    let scope = ResourceLinkScope::catalog(&records, filter, cancel)?;
    read_scoped(&records, filter, cancel, progress, scope, mode)
}

fn read_scoped<'f>(
    records: &[Record<'f>],
    filter: Option<&Filter>,
    cancel: Option<&CancelToken>,
    progress: Option<&(dyn Fn(CatalogProgress) + Sync)>,
    scope: ResourceLinkScope,
    mode: ReadMode<'_, 'f>,
) -> Result<ReadCrafting<'f>, CatalogError> {
    let tracked = records
        .par_iter()
        .map_init(TreeBuilder::new, |trees, record| {
            if cancel.is_some_and(CancelToken::is_cancelled) {
                return Err(CatalogError::Cancelled);
            }
            one(trees, record, filter, cancel, scope, mode)
        })
        .filter_map(Result::transpose)
        .collect::<Result<Vec<_>, _>>()?;
    let mut result = super::assemble::assemble(tracked, mode.origins(), cancel)?;
    inventories(&mut result.ledger);
    if !result.data.pools.is_empty() {
        let found = crafting_scan::reward_sources(
            records,
            &result.data.pools,
            mode.input(),
            cancel,
            progress,
        )?;
        result.data.reward_sources = found.sources;
        result.ledger.merge(found.ledger);
        if found.unreadable > 0 {
            result
                .data
                .warnings
                .push(CraftingWarning::UnreadableRewardRecords(found.unreadable));
        }
    }
    let selected = result
        .blueprints
        .iter()
        .map(|row| row.unknown_range_types.as_slice());
    let interpreted = result
        .data
        .blueprints
        .iter()
        .map(|row| row.unknown_range_types.as_slice());
    if let Some(warning) = crate::crafting::unknown_ranges_warning(interpreted.chain(selected)) {
        result.data.warnings.push(warning);
    }
    let processes = result.data.live_dismantle().count();
    if processes > 1 {
        result
            .data
            .warnings
            .push(CraftingWarning::AmbiguousDismantle(processes));
    }
    Ok(result)
}
