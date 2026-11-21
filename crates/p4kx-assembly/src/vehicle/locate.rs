// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;

use super::elem::Modification;
use super::file_read;
use super::paths;
use super::paths::DIRECTORY;
use super::request;
use super::{VehicleAggregateEvidence, VehicleImplementation};
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
    aggregate: Option<VehicleAggregateEvidence>,
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
    let (path, bytes) = read_definition(
        files,
        class_name,
        definition,
        class_request.as_ref(),
        caller,
        observation,
        trace,
    )?;
    let physical = bytes.entry_origin();
    let mut budget = Budget::new(bytes.len());
    let (mut root, selected_element, aggregate) =
        file_read::base(&path, bytes, &mut budget, modification, observation)?;
    let mut issues = Vec::new();

    let selected = (!modification.is_empty()).then(|| modification.to_string());
    let selection = Selection {
        path: &path,
        stem: paths::file_stem(definition),
        class_name,
        definition,
        modification,
        selected: selected_element.as_ref(),
        class_request: class_request.as_ref(),
        caller,
        aggregate,
    };
    let modified = modification::apply(
        files,
        &mut root,
        selection,
        &mut budget,
        &mut issues,
        observation,
        trace,
    )?;
    Ok(project(
        path,
        selected,
        &root,
        modified,
        issues,
        physical,
        observation,
    ))
}

fn project(
    path: String,
    selected: Option<String>,
    root: &Element,
    modified: AppliedModification,
    issues: Vec<LoadoutError>,
    physical: Option<p4kx_extraction::EntryOrigin>,
    observation: Option<&Observation<'_>>,
) -> VehicleImplementation {
    let mut aggregate = modified.aggregate;
    let totals_observed = observation.is_some_and(Observation::observes_vehicle_totals);
    let ports_observed = observation.is_some_and(Observation::observes_vehicle_port_attributes);
    let port_documents = if ports_observed {
        aggregate.as_ref().map(VehicleAggregateEvidence::checkpoint)
    } else {
        None
    };
    let rolling_documents = if totals_observed {
        aggregate.as_ref().map(VehicleAggregateEvidence::checkpoint)
    } else {
        None
    };
    let totals = if totals_observed {
        aggregate.as_mut()
    } else {
        None
    };
    let parts = super::read::parts(root, totals, ports_observed);
    VehicleImplementation {
        structure: super::structure::digest(root),
        declarations: super::origin::DeclarationOrigins {
            aggregate,
            totals_observed,
            port_documents,
            port_parts: parts.selectors,
            port_occurrences: parts.port_occurrences,
            rolling_documents,
            rolling_parts: parts.rolling_selectors,
            base: physical,
            patch: modified.patch_origin,
            patch_route: modified.patch_route,
        },
        path,
        patch_path: modified.patch_path,
        modification: selected,
        report: modified.report,
        elems: modified.elems,
        name: root.attribute("name").unwrap_or_default().to_owned(),
        port_paths: parts.paths,
        parts: parts.parts,
        rolling_parts: parts.rolling_parts,
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
    class_name: &str,
    definition: &LoadoutPath,
    class_request: &LoadoutPath,
    caller: &LoadoutRequest,
    observation: Option<&Observation<'_>>,
    trace: &mut FileTrace,
) -> Result<(String, Document), LoadoutError> {
    let candidates = definition_candidates(
        class_name,
        definition,
        class_request,
        paths::file_stem(definition),
        observation,
    )?;
    let shown = if definition.is_empty() {
        class_request
    } else {
        definition
    };
    let first = trace.position();
    match file_read::first(
        files,
        &candidates,
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
