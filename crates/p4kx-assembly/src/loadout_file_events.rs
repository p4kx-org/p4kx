// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Deref;

use crate::loadout_file_trace::FileTrace;
use crate::resolver_observation::{self, LoadoutReadEvent, Observation};
use crate::{
    LoadoutError, LoadoutFileError, LoadoutFileObservation, LoadoutFiles, LoadoutPath,
    LoadoutRequest,
};

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

pub(crate) struct Document {
    bytes: Vec<u8>,
    observed: Option<ObservedDocument>,
}

struct ObservedDocument {
    observation: LoadoutFileObservation,
    request: LoadoutPath,
    caller: LoadoutRequest,
}

impl Document {
    pub(crate) fn observation(&self) -> Option<&LoadoutFileObservation> {
        self.observed.as_ref().map(|observed| &observed.observation)
    }
}

impl Deref for Document {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.bytes
    }
}

pub(crate) fn read(
    files: &dyn LoadoutFiles,
    request: &LoadoutPath,
    caller: &LoadoutRequest,
    kind: LoadoutDocumentKind,
    observation: Option<&Observation<'_>>,
    trace: &mut FileTrace,
) -> Result<Document, LoadoutFileError> {
    if observation.is_some_and(Observation::stopped) {
        return Err(LoadoutFileError::ObservationStopped);
    }
    let Some(observer) = observation else {
        return files.read(request).map(|bytes| Document {
            bytes,
            observed: None,
        });
    };
    let (source, outcome) = files.read_observed(request).into_parts();
    let document = trace.capture(source, kind, &outcome, request);
    resolver_observation::notify(
        observation,
        LoadoutReadEvent::FileRead {
            observation: &document,
            request,
            caller,
        },
    )
    .map_err(|_| LoadoutFileError::ObservationStopped)?;
    if matches!(&outcome, Err(LoadoutFileError::SourceEvidenceUnavailable)) {
        observer.stop();
        return Err(LoadoutFileError::SourceEvidenceUnavailable);
    }
    outcome.map(|bytes| Document {
        bytes,
        observed: Some(ObservedDocument {
            observation: document,
            request: request.clone(),
            caller: caller.clone(),
        }),
    })
}

pub(crate) fn parsed(
    document: Document,
    outcome: Result<(), &str>,
    observation: Option<&Observation<'_>>,
) -> Result<(), LoadoutError> {
    if observation.is_some_and(Observation::stopped) {
        return Err(LoadoutError::ObservationStopped);
    }
    if let Some(original) = document.observed {
        let _outcome = original.observation.projected(outcome);
        resolver_observation::notify(
            observation,
            LoadoutReadEvent::FileProjection {
                observation: &original.observation,
                request: &original.request,
                caller: &original.caller,
            },
        )?;
    }
    Ok(())
}
