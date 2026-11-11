// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::elem::{self, Modification};
use super::locate;
use super::patch::{self, ModificationReport};
use crate::loadout_file_events::Document;
use crate::loadout_file_trace::FileTrace;
use crate::markup::{self, Budget, Element, MAX_ELEMENT_DEPTH};
use crate::resolver_observation::{self, LoadoutReadEvent, Observation};
use crate::{
    LoadoutDocumentKind, LoadoutError, LoadoutFileError, LoadoutFiles, LoadoutPath, LoadoutRequest,
};
pub(super) fn base(
    path: &str,
    document: Document,
    budget: &mut Budget,
    requested: &str,
    observation: Option<&Observation<'_>>,
) -> Result<(Element, Option<Modification>), LoadoutError> {
    let result = markup::parse_observed(&document, budget, observation, |root, original, scope| {
        if !root.name.eq_ignore_ascii_case("Vehicle") {
            return Err(format!("its root element is {}, not Vehicle", root.name));
        }
        let selected = if requested.is_empty() {
            None
        } else {
            elem::find_modification(original, scope, requested)?
        };
        Ok((root, selected))
    });
    crate::loadout_file_events::parsed(
        document,
        result.as_ref().map(|_| ()).map_err(String::as_str),
        observation,
    )?;
    result.map_err(|reason| locate::issue(path, reason))
}

/// Parses the source document and applies its patch to `root`, within `budget`.
pub(super) fn patch(
    root: &mut Element,
    source: Document,
    budget: &mut Budget,
    observation: Option<&Observation<'_>>,
) -> Result<ModificationReport, String> {
    let document = match markup::parse_observed(&source, budget, observation, |root, _, _| Ok(root))
    {
        Err(error) if budget.exhausted() => {
            resolver_observation::notify(observation, LoadoutReadEvent::InventoryIncomplete)
                .map_err(|error| error.to_string())?;
            return Err(error);
        }
        result => result,
    };
    crate::loadout_file_events::parsed(
        source,
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
    candidates: &[LoadoutPath],
    caller: &LoadoutRequest,
    kind: LoadoutDocumentKind,
    observation: Option<&Observation<'_>>,
    trace: &mut FileTrace,
) -> Result<Option<(String, Document)>, LoadoutError> {
    for path in candidates {
        match crate::loadout_file_events::read(files, path, caller, kind, observation, trace) {
            Ok(bytes) => return Ok(Some((path.to_string(), bytes))),
            Err(LoadoutFileError::NotFound) => {}
            Err(_) if observation.is_some_and(Observation::stopped) => {
                return Err(LoadoutError::ObservationStopped);
            }
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
