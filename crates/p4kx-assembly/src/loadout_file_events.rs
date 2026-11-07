// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::resolver_observation::{self, LoadoutReadEvent, Observation};
use crate::{LoadoutError, LoadoutFileError, LoadoutFiles};

/// The intrinsic source document interpreted by a loadout reader.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LoadoutDocumentKind {
    /// An XML loadout with its authored port entries and include declarations.
    XmlLoadout,
    /// The vehicle implementation's XML document.
    VehicleImplementation,
    /// A vehicle modification patch's XML document.
    VehiclePatch,
}

/// The exact resolved file identity or a configured target with no physical Entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoadoutFileIdentity<'a> {
    /// The actual Archive Entry index, absent for a missing or non-Archive source.
    pub entry_index: Option<usize>,
    /// The actual canonical Entry path, or the configured path when unresolved.
    pub path: &'a str,
}

fn identity<'a>(files: &'a dyn LoadoutFiles, path: &'a str) -> LoadoutFileIdentity<'a> {
    files.identity(path).unwrap_or(LoadoutFileIdentity {
        entry_index: None,
        path,
    })
}

pub(crate) fn read(
    files: &dyn LoadoutFiles,
    path: &str,
    kind: LoadoutDocumentKind,
    observation: Option<&Observation>,
) -> Result<Vec<u8>, LoadoutFileError> {
    if observation.is_some_and(Observation::stopped) {
        return Err(LoadoutFileError::ObservationStopped);
    }
    let result = files.read(path);
    if observation.is_some() {
        resolver_observation::notify(
            observation,
            LoadoutReadEvent::FileRead {
                source: identity(files, path),
                kind,
                outcome: result.as_ref().map(|_| ()),
            },
        )
        .map_err(|_| LoadoutFileError::ObservationStopped)?;
    }
    result
}

pub(crate) fn parsed(
    files: &dyn LoadoutFiles,
    path: &str,
    kind: LoadoutDocumentKind,
    outcome: Result<(), &str>,
    observation: Option<&Observation>,
) -> Result<(), LoadoutError> {
    if observation.is_some() {
        resolver_observation::notify(
            observation,
            LoadoutReadEvent::FileProjection {
                source: identity(files, path),
                kind,
                outcome,
            },
        )?;
    }
    Ok(())
}
