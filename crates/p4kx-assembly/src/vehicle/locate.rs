// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::VehicleImplementation;
use super::file_read;
use super::paths;
use super::paths::DIRECTORY;
use crate::loadout_file_events::Document;
use crate::loadout_file_trace::FileTrace;
use crate::markup::Budget;
use crate::resolver_observation::Observation;
use crate::{LoadoutDocumentKind, LoadoutError, LoadoutFiles};
use modification::AppliedModification;

#[path = "locate_modification.rs"]
mod modification;

pub(super) fn issue(path: &str, reason: impl Into<String>) -> LoadoutError {
    LoadoutError::VehicleImplementation {
        path: path.to_owned(),
        reason: reason.into(),
    }
}

fn missing(path: &str) -> LoadoutError {
    LoadoutError::MissingVehicleImplementation {
        path: path.to_owned(),
    }
}
/// What selects the Modification: the implementation file, its name, the Entity's class name and
/// the Modification's name.
struct Selection<'a> {
    path: &'a str,
    stem: &'a str,
    class_name: &'a str,
    modification: &'a str,
}

/// Reads the vehicle implementation an Entity names, with the Modification it selects.
///
/// The file is the Record's `vehicleDefinition`; when the Archive has none of that path, the
/// file of the same name in the implementation folder, then the file named after the Entity's
/// class name. The Modification's patch file is its `patchFile`, else
/// `Modifications/<definition name>_<modification>.xml`, else `Modifications/<class name>.xml`.
/// Every path must lie below the implementation folder. A patch file or Modification that cannot
/// be found, read or applied within the budget is an issue of the result; the implementation
/// without it is still returned.
pub(crate) fn load(
    files: &dyn LoadoutFiles,
    class_name: &str,
    definition: &str,
    modification: &str,
) -> Result<VehicleImplementation, LoadoutError> {
    load_observed(
        files,
        class_name,
        definition,
        modification,
        None,
        &mut FileTrace::default(),
    )
}

pub(crate) fn load_observed(
    files: &dyn LoadoutFiles,
    class_name: &str,
    definition: &str,
    modification: &str,
    observation: Option<&Observation>,
    trace: &mut FileTrace,
) -> Result<VehicleImplementation, LoadoutError> {
    let stem = paths::file_stem(definition);
    let candidates = definition_candidates(class_name, definition, stem);
    let shown = if definition.is_empty() {
        class_name
    } else {
        definition
    };
    let (path, bytes) = read_definition(files, &candidates, shown, observation, trace)?;
    let mut budget = Budget::new(bytes.len());
    let mut root = file_read::base(&path, bytes, &mut budget, observation)?;
    let mut issues = Vec::new();

    let selected = (!modification.is_empty()).then(|| modification.to_owned());
    let modified = if let Some(name) = &selected {
        let selection = Selection {
            path: &path,
            stem,
            class_name,
            modification: name,
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
    let parts = super::read::parts(&root);
    Ok(VehicleImplementation {
        structure: super::structure::digest(&root),
        path,
        patch_path: modified.patch_path,
        modification: selected,
        report: modified.report,
        elems: modified.elems,
        name: root.attribute("name").unwrap_or_default().to_owned(),
        port_paths: super::read::port_paths(&parts),
        parts,
        issues,
    })
}

/// Returns the candidates for the implementation file: the definition, the file of its name in
/// the implementation folder and the file named after the class.
fn definition_candidates(class_name: &str, definition: &str, stem: &str) -> Vec<String> {
    let mut candidates = Vec::new();
    paths::add_candidate(&mut candidates, definition);
    if paths::usable_name(stem) {
        paths::add_candidate(&mut candidates, &format!("{DIRECTORY}/{stem}.xml"));
    }
    if paths::usable_name(class_name) {
        paths::add_candidate(&mut candidates, &format!("{DIRECTORY}/{class_name}.xml"));
    }
    candidates
}

fn read_definition(
    files: &dyn LoadoutFiles,
    candidates: &[String],
    shown: &str,
    observation: Option<&Observation>,
    trace: &mut FileTrace,
) -> Result<(String, Document), LoadoutError> {
    match file_read::first(
        files,
        candidates,
        LoadoutDocumentKind::VehicleImplementation,
        observation,
        trace,
    ) {
        Ok(Some(found)) => Ok(found),
        Ok(None) => Err(if candidates.is_empty() {
            issue(shown, format!("the path is not below {DIRECTORY}"))
        } else {
            missing(shown)
        }),
        Err(error) => Err(error),
    }
}
