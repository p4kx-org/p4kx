// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::locate;
use super::patch::{self, ModificationReport};
use crate::markup::{self, Budget, Element, MAX_ELEMENT_DEPTH};
use crate::resolver_observation::{self, LoadoutReadEvent, Observation};
use crate::{LoadoutDocumentKind, LoadoutError, LoadoutFileError, LoadoutFiles};
pub(super) fn base(
    files: &dyn LoadoutFiles,
    path: &str,
    bytes: &[u8],
    budget: &mut Budget,
    observation: Option<&Observation>,
) -> Result<Element, LoadoutError> {
    let result = markup::parse(bytes, budget).and_then(|root| {
        if root.name.eq_ignore_ascii_case("Vehicle") {
            Ok(root)
        } else {
            Err(format!("its root element is {}, not Vehicle", root.name))
        }
    });
    crate::loadout_file_events::parsed(
        files,
        path,
        LoadoutDocumentKind::VehicleImplementation,
        result.as_ref().map(|_| ()).map_err(String::as_str),
        observation,
    )?;
    result.map_err(|reason| locate::issue(path, reason))
}

/// Parses the patch `bytes` and applies it to `root`, within `budget`.
pub(super) fn patch(
    root: &mut Element,
    bytes: &[u8],
    budget: &mut Budget,
    files: &dyn LoadoutFiles,
    path: &str,
    observation: Option<&Observation>,
) -> Result<ModificationReport, String> {
    let document = match markup::parse(bytes, budget) {
        Err(error) if budget.exhausted() => {
            resolver_observation::notify(observation, LoadoutReadEvent::InventoryIncomplete)
                .map_err(|error| error.to_string())?;
            return Err(error);
        }
        result => result,
    };
    crate::loadout_file_events::parsed(
        files,
        path,
        LoadoutDocumentKind::VehiclePatch,
        document.as_ref().map(|_| ()).map_err(String::as_str),
        observation,
    )
    .map_err(|error| error.to_string())?;
    let document = document?;
    let done = patch::apply_patch(root, &document, budget)?;
    if markup::depth(root) > MAX_ELEMENT_DEPTH {
        return Err(markup::too_deep());
    }
    Ok(done)
}

/// Returns the first of `candidates` the files hold with its bytes, `None` when none exists,
/// or why a file that exists cannot be read.
pub(super) fn first(
    files: &dyn LoadoutFiles,
    candidates: &[String],
    kind: LoadoutDocumentKind,
    observation: Option<&Observation>,
) -> Result<Option<(String, Vec<u8>)>, LoadoutError> {
    for path in candidates {
        match crate::loadout_file_events::read(files, path, kind, observation) {
            Ok(bytes) => return Ok(Some((path.clone(), bytes))),
            Err(LoadoutFileError::NotFound) => {}
            Err(LoadoutFileError::ObservationStopped) => {
                return Err(LoadoutError::ObservationStopped);
            }
            Err(error) => return Err(locate::issue(path, error.to_string())),
        }
    }
    Ok(None)
}

#[cfg(test)]
#[path = "file_read_tests.rs"]
mod tests;
