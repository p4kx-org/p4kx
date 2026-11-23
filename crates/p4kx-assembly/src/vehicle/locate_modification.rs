// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::Selection;
use crate::loadout_file_events::Document;
use crate::loadout_file_trace::FileTrace;
use crate::markup::{Budget, Element};
use crate::resolver_observation::{self, LoadoutReadEvent, Observation};
use crate::vehicle::ElemOverride;
use crate::vehicle::elem::{Modification, ModificationPatch};
use crate::vehicle::file_read;
use crate::vehicle::origin::PatchRoute;
use crate::vehicle::patch::{self, ModificationReport};
use crate::vehicle::paths;
use crate::vehicle::paths::DIRECTORY;
use crate::{
    LoadoutDocumentKind, LoadoutError, LoadoutFailurePath, LoadoutFiles, LoadoutPath,
    LoadoutPathOperation,
};
use crate::{VehicleAggregateEvidence, VehicleAggregateIssue};

const LISTED: usize = 8;

/// The patch and element overrides retained by one selected Modification.
#[derive(Default)]
pub(super) struct AppliedModification {
    pub(super) aggregate: Option<VehicleAggregateEvidence>,
    pub(super) patch_path: Option<String>,
    pub(super) patch_origin: Option<p4kx_extraction::EntryOrigin>,
    pub(super) patch_route: Option<PatchRoute>,
    pub(super) patch_file: Option<crate::LoadoutFileObservation>,
    pub(super) report: Option<ModificationReport>,
    pub(super) elems: Vec<ElemOverride>,
}

/// Applies the selected patch and element overrides within the shared budget.
pub(super) fn apply(
    files: &dyn LoadoutFiles,
    root: &mut Element,
    mut selection: Selection<'_>,
    budget: &mut Budget,
    issues: &mut Vec<LoadoutError>,
    observation: Option<&Observation<'_>>,
    trace: &mut FileTrace,
) -> Result<AppliedModification, LoadoutError> {
    let mut modified = AppliedModification {
        aggregate: selection.aggregate.take(),
        ..AppliedModification::default()
    };
    if selection.modification.is_empty() {
        return Ok(modified);
    }
    let authored = selection
        .selected
        .and_then(|selected| selected.patch.as_ref());
    let (candidates, explicit) = patch_candidates(
        &selection,
        authored,
        issues,
        observation,
        modified.aggregate.as_mut(),
    )?;

    let first = trace.position();
    let read = file_read::first(
        files,
        &candidates,
        selection.caller,
        LoadoutDocumentKind::VehiclePatch,
        observation,
        trace,
    );
    let route = explicit_patch_route(&read, explicit.as_ref().zip(authored));
    if let Some(wanted) = explicit.filter(|wanted| match &read {
        Ok(Some((path, _))) => !wanted.eq_ignore_ascii_case(path),
        Ok(None) => true,
        Err(_) => false,
    }) {
        if let Some(aggregate) = &mut modified.aggregate {
            aggregate.issue(VehicleAggregateIssue::ExplicitPatchMissing);
        }
        issues.push(super::missing(LoadoutFailurePath::from_attempts(
            wanted,
            trace.attempts(first, 1),
        )));
    }
    apply_read_patch(root, read, budget, issues, observation, &mut modified)?;
    modified.patch_route = route.filter(|_| modified.patch_path.is_some());
    apply_elements(
        root,
        selection.selected,
        &selection,
        budget,
        issues,
        observation,
        &mut modified,
    )?;
    Ok(modified)
}

fn explicit_patch_route(
    read: &Result<Option<(String, Document)>, LoadoutError>,
    authored: Option<(&LoadoutPath, &ModificationPatch)>,
) -> Option<PatchRoute> {
    let ((selected, document), (requested, original)) =
        read.as_ref().ok()?.as_ref().zip(authored)?;
    if !requested.eq_ignore_ascii_case(selected) {
        return None;
    }
    Some(PatchRoute {
        file: document.observation()?.clone(),
        selector: original.selector.clone()?,
    })
}

fn apply_read_patch(
    root: &mut Element,
    read: Result<Option<(String, Document)>, LoadoutError>,
    budget: &mut Budget,
    issues: &mut Vec<LoadoutError>,
    observation: Option<&Observation<'_>>,
    modified: &mut AppliedModification,
) -> Result<(), LoadoutError> {
    match read {
        Ok(Some((path, bytes))) => {
            let physical = bytes.entry_origin();
            let file = bytes.observation().cloned();
            budget.grow(bytes.len());
            if let Some(done) = apply_patch(
                root,
                bytes,
                &path,
                budget,
                issues,
                observation,
                modified.aggregate.as_mut(),
            )? {
                modified.patch_path = Some(path);
                modified.patch_origin = physical;
                modified.patch_file = file;
                modified.report = Some(done);
            }
        }
        Ok(None) => {}
        Err(LoadoutError::ObservationStopped) => return Err(LoadoutError::ObservationStopped),
        Err(error) => {
            if let Some(aggregate) = &mut modified.aggregate {
                aggregate.issue(VehicleAggregateIssue::PatchReadFailed);
            }
            issues.push(error);
        }
    }
    Ok(())
}

/// Returns the candidates for the patch file, and the one the Modification names itself.
fn patch_candidates(
    selection: &Selection<'_>,
    authored: Option<&ModificationPatch>,
    issues: &mut Vec<LoadoutError>,
    observation: Option<&Observation<'_>>,
    aggregate: Option<&mut VehicleAggregateEvidence>,
) -> Result<(Vec<LoadoutPath>, Option<LoadoutPath>), LoadoutError> {
    let mut candidates = Vec::new();
    let mut explicit = None;
    if let Some(authored) = authored {
        if let Some(path) = paths::confine(&paths::patch_path(&authored.path)) {
            let path = LoadoutPath::derived(
                path,
                LoadoutPathOperation::VehiclePatch,
                &[&authored.path, selection.modification, &authored.name],
                observation,
            )?;
            candidates.push(path.clone());
            explicit = Some(path);
        } else {
            if let Some(aggregate) = aggregate {
                aggregate.issue(VehicleAggregateIssue::PatchPathRejected);
            }
            issues.push(super::issue(
                &authored.path,
                format!("the patch file is not below {DIRECTORY}"),
            ));
        }
    }
    if paths::usable_name(selection.stem) && paths::usable_name(selection.modification) {
        crate::vehicle::request::add(
            &mut candidates,
            &format!(
                "{DIRECTORY}/Modifications/{}_{}.xml",
                selection.stem, selection.modification
            ),
            LoadoutPathOperation::VehiclePatchStem,
            &[selection.definition, selection.modification],
            observation,
        )?;
    }
    if paths::usable_name(selection.class_name) {
        crate::vehicle::request::add(
            &mut candidates,
            &format!("{DIRECTORY}/Modifications/{}.xml", selection.class_name),
            LoadoutPathOperation::VehiclePatchClass,
            &[selection.class_request],
            observation,
        )?;
    }
    Ok((candidates, explicit))
}

/// Runs `run` on `root` and restores `root` as it was when `run` fails; the copy that makes this
/// possible must fit the budget, which it does not spend.
fn guarded<T>(
    root: &mut Element,
    budget: &mut Budget,
    observation: Option<&Observation<'_>>,
    mut aggregate: Option<&mut VehicleAggregateEvidence>,
    run: impl FnOnce(
        &mut Element,
        &mut Budget,
        Option<&mut VehicleAggregateEvidence>,
    ) -> Result<T, String>,
) -> Result<T, String> {
    if let Err(error) = budget.check_copy(root) {
        resolver_observation::notify(observation, LoadoutReadEvent::InventoryIncomplete)
            .map_err(|error| error.to_string())?;
        return Err(error);
    }
    let backup = root.clone();
    let checkpoint = aggregate.as_ref().map(|scope| scope.checkpoint());
    let result = run(&mut *root, budget, aggregate.as_deref_mut());
    if result.is_err() {
        *root = backup;
        if let (Some(aggregate), Some(checkpoint)) = (aggregate, checkpoint) {
            aggregate.restore(checkpoint);
        }
    }
    if result.is_err() && budget.exhausted() {
        resolver_observation::notify(observation, LoadoutReadEvent::InventoryIncomplete)
            .map_err(|error| error.to_string())?;
    }
    result
}

fn report_issues(path: &str, report: &ModificationReport, issues: &mut Vec<LoadoutError>) {
    let listed = |names: &[String]| {
        let shown: Vec<&str> = names.iter().take(LISTED).map(String::as_str).collect();
        let more = names.len().saturating_sub(LISTED);
        if more == 0 {
            shown.join(", ")
        } else {
            format!("{} and {more} more", shown.join(", "))
        }
    };
    if !report.unmatched_children.is_empty() {
        issues.push(super::issue(
            path,
            format!(
                "the patch replaces {} that the base lacks: {}",
                report.unmatched_children.len(),
                listed(&report.unmatched_children)
            ),
        ));
    }
    if !report.unmatched_ids.is_empty() {
        issues.push(super::issue(
            path,
            format!(
                "the patch names {} ids that no element has: {}",
                report.unmatched_ids.len(),
                listed(&report.unmatched_ids)
            ),
        ));
    }
}

fn apply_patch(
    root: &mut Element,
    bytes: Document,
    path: &str,
    budget: &mut Budget,
    issues: &mut Vec<LoadoutError>,
    observation: Option<&Observation<'_>>,
    mut aggregate: Option<&mut VehicleAggregateEvidence>,
) -> Result<Option<ModificationReport>, LoadoutError> {
    match guarded(
        root,
        budget,
        observation,
        aggregate.as_deref_mut(),
        |root, budget, aggregate| file_read::patch(root, bytes, budget, observation, aggregate),
    ) {
        Ok(done) => {
            report_issues(path, &done, issues);
            Ok(Some(done))
        }
        Err(_) if observation.is_some_and(Observation::stopped) => {
            Err(LoadoutError::ObservationStopped)
        }
        Err(reason) => {
            if let Some(aggregate) = aggregate {
                aggregate.issue(VehicleAggregateIssue::PatchApplicationFailed);
            }
            issues.push(super::issue(path, reason));
            Ok(None)
        }
    }
}
fn apply_elements(
    root: &mut Element,
    found: Option<&Modification>,
    selection: &Selection<'_>,
    budget: &mut Budget,
    issues: &mut Vec<LoadoutError>,
    observation: Option<&Observation<'_>>,
    modified: &mut AppliedModification,
) -> Result<(), LoadoutError> {
    match found {
        Some(found) => match guarded(
            root,
            budget,
            observation,
            modified.aggregate.as_mut(),
            |root, budget, aggregate| patch::apply_elems(root, &found.elems, budget, aggregate),
        ) {
            Ok(missed) => {
                modified.elems = found
                    .elems
                    .iter()
                    .filter(|elem| !missed.contains(&elem.id_ref))
                    .cloned()
                    .collect();
                modified
                    .report
                    .get_or_insert_with(ModificationReport::default)
                    .unmatched_elems = missed;
            }
            Err(_) if observation.is_some_and(Observation::stopped) => {
                return Err(LoadoutError::ObservationStopped);
            }
            Err(error) => {
                if let Some(aggregate) = &mut modified.aggregate {
                    aggregate.issue(VehicleAggregateIssue::ElementOverridesFailed);
                }
                issues.push(super::issue(selection.path, error));
            }
        },
        None if modified.patch_path.is_none() => {
            if let Some(aggregate) = &mut modified.aggregate {
                aggregate.issue(VehicleAggregateIssue::ModificationUnavailable);
            }
            issues.push(super::issue(
                selection.path,
                format!("it names no Modification {}", selection.modification),
            ));
        }
        None => {}
    }
    Ok(())
}

#[cfg(test)]
#[path = "aggregate_rollback_tests.rs"]
mod aggregate_rollback_tests;
