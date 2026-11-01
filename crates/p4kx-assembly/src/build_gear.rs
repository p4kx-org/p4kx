// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;
use std::sync::Arc;

use p4kx_model::Model;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};

use crate::build::issue;
use crate::build_head_library::use_head_libraries;
use crate::build_models::{Models, Outcome, fall_back, read_models};
use crate::flatten::Draft;
use crate::{AssemblyError, AssemblyOptions, Issue, IssueKind, ModelFailure, ModelProvider};

/// Reads the model of every part, falls back from a missing sub-geometry to the main geometry and
/// poses the landing gear.
pub(crate) fn load_models(
    drafts: &mut [Draft<'_>],
    provider: &dyn ModelProvider,
    options: &AssemblyOptions,
    issues: &mut Vec<Issue>,
) -> Result<Models, AssemblyError> {
    let mut models = read_models(drafts, provider, options)?;
    fall_back(drafts, &mut models, provider, options, issues)?;
    use_head_libraries(drafts, provider, issues);
    pose_gears(drafts, &mut models, provider, issues)?;
    Ok(models)
}

/// Poses the model of every landing gear at the end of its deploy clip, once per distinct model.
///
/// A gear whose model cannot be posed keeps the pose of its model, and a note on the gear says why.
pub(crate) fn pose_gears(
    drafts: &[Draft<'_>],
    models: &mut Models,
    provider: &dyn ModelProvider,
    issues: &mut Vec<Issue>,
) -> Result<(), AssemblyError> {
    let mut seen = HashSet::new();
    let wanted: Vec<(String, &str, Arc<Model>)> = drafts
        .iter()
        .filter(|draft| draft.gear.is_some())
        .filter_map(|draft| {
            let key = draft.model_key()?;
            let path = draft.geometry_path()?;
            match models.get(&key) {
                Some(Outcome::Loaded(loaded)) if seen.insert(key.clone()) => {
                    Some((key, path, Arc::clone(&loaded.model)))
                }
                _ => None,
            }
        })
        .collect();
    let posed: Vec<(String, Result<Option<Model>, ModelFailure>)> = wanted
        .par_iter()
        .map(|(key, path, model)| (key.clone(), provider.deployed(path, model)))
        .collect();
    let mut reasons: Vec<(String, String)> = Vec::new();
    for (key, result) in posed {
        match result {
            Ok(Some(model)) => {
                if let Some(Outcome::Loaded(loaded)) = models.get_mut(&key) {
                    loaded.model = Arc::new(model);
                }
            }
            Ok(None) => reasons.push((key, "the Archive has no deploy clip for it".to_owned())),
            Err(ModelFailure::Fatal(source)) => return Err(AssemblyError::Provider(source)),
            Err(ModelFailure::Missing) => {
                reasons.push((key, "its deploy clip is not in the Archive".to_owned()));
            }
            Err(other) => reasons.push((key, other.to_string())),
        }
    }
    for draft in drafts.iter().filter(|draft| draft.gear.is_some()) {
        let Some(key) = draft.model_key() else {
            continue;
        };
        if let Some((_, reason)) = reasons.iter().find(|(found, _)| *found == key) {
            let reason = reason.clone();
            issues.push(issue(draft, IssueKind::GearNotDeployed { reason }));
        }
    }
    Ok(())
}
