// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::Selection;
use crate::loadout_file_events::Document;
use crate::loadout_file_trace::FileTrace;
use crate::markup::{Budget, Element};
use crate::resolver_observation::{self, LoadoutReadEvent, Observation};
use crate::vehicle::ElemOverride;
use crate::vehicle::elem::{self, Modification};
use crate::vehicle::file_read;
use crate::vehicle::patch::{self, ModificationReport};
use crate::vehicle::paths;
use crate::vehicle::paths::DIRECTORY;
use crate::{LoadoutDocumentKind, LoadoutError, LoadoutFiles};

const LISTED: usize = 8;

/// The patch and element overrides retained by one selected Modification.
#[derive(Default)]
pub(super) struct AppliedModification {
    pub(super) patch_path: Option<String>,
    pub(super) report: Option<ModificationReport>,
    pub(super) elems: Vec<ElemOverride>,
}

/// Applies the selected patch and element overrides within the shared budget.
pub(super) fn apply(
    files: &dyn LoadoutFiles,
    root: &mut Element,
    selection: &Selection<'_>,
    budget: &mut Budget,
    issues: &mut Vec<LoadoutError>,
    observation: Option<&Observation>,
    trace: &mut FileTrace,
) -> Result<AppliedModification, LoadoutError> {
    let mut modified = AppliedModification::default();
    let found = elem::find_modification(root, selection.modification);
    let patch_file = found.as_ref().and_then(|found| found.patch_file.as_deref());
    let (candidates, explicit) = patch_candidates(selection, patch_file, issues);

    match file_read::first(
        files,
        &candidates,
        LoadoutDocumentKind::VehiclePatch,
        observation,
        trace,
    ) {
        Ok(Some((path, bytes))) => {
            if let Some(wanted) = explicit.as_ref().filter(|w| !w.eq_ignore_ascii_case(&path)) {
                issues.push(super::missing(wanted));
            }
            budget.grow(bytes.len());
            if let Some(done) = apply_patch(root, bytes, &path, budget, issues, observation)? {
                modified.patch_path = Some(path);
                modified.report = Some(done);
            }
        }
        Ok(None) => {
            if let Some(wanted) = &explicit {
                issues.push(super::missing(wanted));
            }
        }
        Err(LoadoutError::ObservationStopped) => return Err(LoadoutError::ObservationStopped),
        Err(error) => issues.push(error),
    }
    apply_elements(
        root,
        found.as_ref(),
        selection,
        budget,
        issues,
        observation,
        &mut modified,
    )?;
    Ok(modified)
}

/// Returns the candidates for the patch file, and the one the Modification names itself.
fn patch_candidates(
    selection: &Selection<'_>,
    patch_file: Option<&str>,
    issues: &mut Vec<LoadoutError>,
) -> (Vec<String>, Option<String>) {
    let mut candidates = Vec::new();
    let mut explicit = None;
    if let Some(file) = patch_file {
        if let Some(path) = paths::confine(&paths::patch_path(file)) {
            paths::push_unique(&mut candidates, &path);
            explicit = Some(path);
        } else {
            issues.push(super::issue(
                file,
                format!("the patch file is not below {DIRECTORY}"),
            ));
        }
    }
    if paths::usable_name(selection.stem) && paths::usable_name(selection.modification) {
        paths::add_candidate(
            &mut candidates,
            &format!(
                "{DIRECTORY}/Modifications/{}_{}.xml",
                selection.stem, selection.modification
            ),
        );
    }
    if paths::usable_name(selection.class_name) {
        paths::add_candidate(
            &mut candidates,
            &format!("{DIRECTORY}/Modifications/{}.xml", selection.class_name),
        );
    }
    (candidates, explicit)
}

/// Runs `run` on `root` and restores `root` as it was when `run` fails; the copy that makes this
/// possible must fit the budget, which it does not spend.
fn guarded<T>(
    root: &mut Element,
    budget: &mut Budget,
    observation: Option<&Observation>,
    run: impl FnOnce(&mut Element, &mut Budget) -> Result<T, String>,
) -> Result<T, String> {
    if let Err(error) = budget.check_copy(root) {
        resolver_observation::notify(observation, LoadoutReadEvent::InventoryIncomplete)
            .map_err(|error| error.to_string())?;
        return Err(error);
    }
    let backup = root.clone();
    let result = run(&mut *root, budget).inspect_err(|_| *root = backup);
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
    observation: Option<&Observation>,
) -> Result<Option<ModificationReport>, LoadoutError> {
    match guarded(root, budget, observation, |root, budget| {
        file_read::patch(root, bytes, budget, observation)
    }) {
        Ok(done) => {
            report_issues(path, &done, issues);
            Ok(Some(done))
        }
        Err(_) if observation.is_some_and(Observation::stopped) => {
            Err(LoadoutError::ObservationStopped)
        }
        Err(reason) => {
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
    observation: Option<&Observation>,
    modified: &mut AppliedModification,
) -> Result<(), LoadoutError> {
    match found {
        Some(found) => match guarded(root, budget, observation, |root, budget| {
            patch::apply_elems(root, &found.elems, budget)
        }) {
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
            Err(error) => issues.push(super::issue(selection.path, error)),
        },
        None if modified.patch_path.is_none() => issues.push(super::issue(
            selection.path,
            format!("it names no Modification {}", selection.modification),
        )),
        None => {}
    }
    Ok(())
}
