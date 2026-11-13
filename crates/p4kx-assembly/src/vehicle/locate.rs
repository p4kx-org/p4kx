// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;

use super::VehicleImplementation;
use super::elem::Modification;
use super::file_read;
use super::paths;
use super::paths::DIRECTORY;
use super::request;
use crate::loadout_file_events::Document;
use crate::loadout_file_trace::FileTrace;
use crate::markup::{Budget, Element};
use crate::resolver_observation::Observation;
use crate::{
    LoadoutDocumentKind, LoadoutError, LoadoutFailurePath, LoadoutFiles, LoadoutPath,
    LoadoutPathOperation, LoadoutRequest,
};
use modification::AppliedModification;

#[path = "locate_modification.rs"]
mod modification;

pub(super) fn issue(path: &str, reason: impl Into<String>) -> LoadoutError {
    LoadoutError::VehicleImplementation {
        path: path.to_owned(),
        reason: reason.into(),
    }
}

const fn missing(path: LoadoutFailurePath) -> LoadoutError {
    LoadoutError::MissingVehicleImplementation { path }
}

struct Selection<'a> {
    path: &'a str,
    stem: &'a str,
    class_name: &'a str,
    definition: &'a LoadoutPath,
    modification: &'a LoadoutPath,
    selected: Option<&'a Modification>,
    class_request: &'a LoadoutPath,
    caller: &'a LoadoutRequest,
}

pub(crate) fn load(
    files: &dyn LoadoutFiles,
    class_name: &str,
    definition: &str,
    modification: &str,
) -> Result<VehicleImplementation, LoadoutError> {
    let caller = LoadoutRequest::Vehicle {
        definition: LoadoutPath::unbound(definition),
        modification: LoadoutPath::unbound(modification),
        class_name: None,
    };
    load_observed(files, class_name, &caller, None, &mut FileTrace::default())
}

pub(crate) fn load_observed(
    files: &dyn LoadoutFiles,
    class_name: &str,
    caller: &LoadoutRequest,
    observation: Option<&Observation<'_>>,
    trace: &mut FileTrace,
) -> Result<VehicleImplementation, LoadoutError> {
    let LoadoutRequest::Vehicle {
        definition,
        modification,
        class_name: record_name,
    } = caller
    else {
        return Err(issue(
            class_name,
            "the vehicle reader requires a vehicle caller",
        ));
    };
    let class_request = record_name.as_ref().map_or_else(
        || Cow::Owned(LoadoutPath::unbound(class_name)),
        Cow::Borrowed,
    );
    let stem = paths::file_stem(definition);
    let candidates = definition_candidates(
        class_name,
        definition,
        class_request.as_ref(),
        stem,
        observation,
    )?;
    let shown = if definition.is_empty() {
        class_request.as_ref()
    } else {
        definition
    };
    let (path, bytes) = read_definition(files, &candidates, shown, caller, observation, trace)?;
    let physical = bytes.entry_origin();
    let mut budget = Budget::new(bytes.len());
    let (mut root, selected_element) =
        file_read::base(&path, bytes, &mut budget, modification, observation)?;
    let mut issues = Vec::new();

    let selected = (!modification.is_empty()).then(|| modification.to_string());
    let modified = if selected.is_some() {
        let selection = Selection {
            path: &path,
            stem,
            class_name,
            definition,
            modification,
            selected: selected_element.as_ref(),
            class_request: class_request.as_ref(),
            caller,
        };
        modification::apply(
            files,
            &mut root,
            &selection,
            &mut budget,
            &mut issues,
            observation,
            trace,
        )?
    } else {
        AppliedModification::default()
    };
    Ok(project(path, selected, &root, modified, issues, physical))
}

fn project(
    path: String,
    selected: Option<String>,
    root: &Element,
    modified: AppliedModification,
    issues: Vec<LoadoutError>,
    physical: Option<p4kx_extraction::EntryOrigin>,
) -> VehicleImplementation {
    let parts = super::read::parts(root);
    VehicleImplementation {
        structure: super::structure::digest(root),
        declarations: super::origin::DeclarationOrigins {
            base: physical,
            patch: modified.patch_origin,
        },
        path,
        patch_path: modified.patch_path,
        modification: selected,
        report: modified.report,
        elems: modified.elems,
        name: root.attribute("name").unwrap_or_default().to_owned(),
        port_paths: super::read::port_paths(&parts),
        parts,
        issues,
    }
}

fn definition_candidates(
    class_name: &str,
    definition: &LoadoutPath,
    class_request: &LoadoutPath,
    stem: &str,
    observation: Option<&Observation<'_>>,
) -> Result<Vec<LoadoutPath>, LoadoutError> {
    let mut candidates = Vec::new();
    request::add(
        &mut candidates,
        definition,
        LoadoutPathOperation::VehicleDefinition,
        &[definition],
        observation,
    )?;
    if paths::usable_name(stem) {
        request::add(
            &mut candidates,
            &format!("{DIRECTORY}/{stem}.xml"),
            LoadoutPathOperation::VehicleDefinitionStem,
            &[definition],
            observation,
        )?;
    }
    if paths::usable_name(class_name) {
        request::add(
            &mut candidates,
            &format!("{DIRECTORY}/{class_name}.xml"),
            LoadoutPathOperation::VehicleClass,
            &[class_request],
            observation,
        )?;
    }
    Ok(candidates)
}

fn read_definition(
    files: &dyn LoadoutFiles,
    candidates: &[LoadoutPath],
    shown: &LoadoutPath,
    caller: &LoadoutRequest,
    observation: Option<&Observation<'_>>,
    trace: &mut FileTrace,
) -> Result<(String, Document), LoadoutError> {
    let first = trace.position();
    match file_read::first(
        files,
        candidates,
        caller,
        LoadoutDocumentKind::VehicleImplementation,
        observation,
        trace,
    ) {
        Ok(Some(found)) => Ok(found),
        Ok(None) => Err(if candidates.is_empty() {
            issue(shown, format!("the path is not below {DIRECTORY}"))
        } else {
            missing(LoadoutFailurePath::from_attempts(
                shown.clone(),
                trace.attempts(first, candidates.len()),
            ))
        }),
        Err(error) => Err(error),
    }
}
