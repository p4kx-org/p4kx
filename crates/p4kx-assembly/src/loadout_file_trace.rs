// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use crate::loadout_xml_use::XmlInclude;
use crate::resolver_observation::{self, LoadoutReadEvent, Observation};
use crate::{
    LoadoutDocumentKind, LoadoutError, LoadoutFileError, LoadoutFileObservation, LoadoutFileSource,
    LoadoutPath, LoadoutRequest,
};

#[derive(Debug, Default)]
pub(crate) struct FileTrace {
    documents: Vec<CapturedDocument>,
    xml: Option<usize>,
}

#[derive(Debug)]
struct CapturedDocument {
    observation: LoadoutFileObservation,
    request: LoadoutPath,
}

impl FileTrace {
    /// Retains the exact admitted source, actual request and completed read outcome.
    pub(crate) fn capture(
        &mut self,
        source: LoadoutFileSource,
        kind: LoadoutDocumentKind,
        outcome: &Result<Vec<u8>, LoadoutFileError>,
        request: &LoadoutPath,
    ) -> LoadoutFileObservation {
        let document = LoadoutFileObservation::new(
            source,
            kind,
            outcome.as_ref().map(|_| ()).map_err(Clone::clone),
        );
        self.documents.push(CapturedDocument {
            observation: document.clone(),
            request: request.clone(),
        });
        document
    }

    pub(crate) fn attempts(
        &self,
        start: usize,
        count: usize,
    ) -> Option<Arc<[LoadoutFileObservation]>> {
        let attempts = self.documents.get(start..)?.get(..count)?;
        Some(
            attempts
                .iter()
                .map(|document| document.observation.clone())
                .collect(),
        )
    }

    pub(crate) const fn position(&self) -> usize {
        self.documents.len()
    }

    pub(crate) fn captured(&self, position: usize) -> Option<usize> {
        self.documents.get(position).map(|_| position)
    }

    pub(crate) fn xml_root(&self) -> Option<&LoadoutFileObservation> {
        self.xml
            .and_then(|index| self.documents.get(index))
            .map(|document| &document.observation)
    }

    pub(crate) const fn set_xml(&mut self, operation: Option<usize>) {
        self.xml = operation;
    }

    pub(crate) fn dependency(
        &self,
        parent: Option<usize>,
        child: Option<usize>,
        dependency: &LoadoutRequest,
        observation: Option<&Observation<'_>>,
    ) -> Result<Option<XmlInclude>, LoadoutError> {
        let Some(observer) = observation else {
            return Ok(None);
        };
        let LoadoutRequest::Xml { path } = dependency else {
            observer.stop();
            return Err(LoadoutError::ObservationStopped);
        };
        let parent = self.original(parent, observer)?;
        let child = self.original(child, observer)?;
        resolver_observation::notify(
            observation,
            LoadoutReadEvent::FileDependency {
                parent: &parent.observation,
                observation: &child.observation,
                request: &child.request,
                dependency,
            },
        )?;
        Ok(Some(XmlInclude {
            parent: parent.observation.clone(),
            child: child.observation.clone(),
            path: path.clone(),
        }))
    }

    pub(crate) fn refused(
        &self,
        parent: Option<usize>,
        dependency: &LoadoutRequest,
        error: &LoadoutError,
        observation: Option<&Observation<'_>>,
    ) -> Result<(), LoadoutError> {
        let Some(observer) = observation else {
            return Ok(());
        };
        let parent = self.original(parent, observer)?;
        resolver_observation::notify(
            observation,
            LoadoutReadEvent::FileDependencyRefused {
                parent: &parent.observation,
                dependency,
                error,
            },
        )
    }

    /// Links today's caller to the retained XML root or the original vehicle operations.
    pub(crate) fn reused(
        &self,
        observation: Option<&Observation<'_>>,
        caller: &LoadoutRequest,
    ) -> Result<(), LoadoutError> {
        if let Some(index) = self.xml {
            if let Some(observer) = observation {
                return Self::notify(self.original(Some(index), observer)?, observation, caller);
            }
            return Ok(());
        }
        for document in &self.documents {
            Self::notify(document, observation, caller)?;
        }
        Ok(())
    }

    fn original(
        &self,
        index: Option<usize>,
        observation: &Observation<'_>,
    ) -> Result<&CapturedDocument, LoadoutError> {
        let Some(document) = index.and_then(|index| self.documents.get(index)) else {
            observation.stop();
            return Err(LoadoutError::ObservationStopped);
        };
        Ok(document)
    }

    fn notify(
        document: &CapturedDocument,
        observation: Option<&Observation<'_>>,
        caller: &LoadoutRequest,
    ) -> Result<(), LoadoutError> {
        resolver_observation::notify(
            observation,
            LoadoutReadEvent::FileReused {
                observation: &document.observation,
                request: &document.request,
                caller,
            },
        )
    }
}
