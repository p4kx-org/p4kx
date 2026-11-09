// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use crate::resolver_observation::{self, LoadoutReadEvent, Observation};
use crate::{
    LoadoutDocumentKind, LoadoutError, LoadoutFileError, LoadoutFileObservation, LoadoutFileSource,
};

#[derive(Debug, Default)]
pub(crate) struct FileTrace {
    documents: Vec<Arc<LoadoutFileObservation>>,
}

impl FileTrace {
    /// Retains the exact admitted source and completed read outcome.
    pub(crate) fn capture(
        &mut self,
        source: LoadoutFileSource,
        kind: LoadoutDocumentKind,
        outcome: &Result<Vec<u8>, LoadoutFileError>,
    ) -> Arc<LoadoutFileObservation> {
        let document = Arc::new(LoadoutFileObservation::new(
            source,
            kind,
            outcome.as_ref().map(|_| ()).map_err(Clone::clone),
        ));
        self.documents.push(Arc::clone(&document));
        document
    }

    /// Observes the original operations without repeating reads or parses.
    pub(crate) fn reused(&self, observation: Option<&Observation>) -> Result<(), LoadoutError> {
        for document in &self.documents {
            resolver_observation::notify(
                observation,
                LoadoutReadEvent::FileReused {
                    observation: document,
                },
            )?;
        }
        Ok(())
    }
}
