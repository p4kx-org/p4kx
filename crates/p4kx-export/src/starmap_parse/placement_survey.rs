// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{FieldOrigin, SourceUse, SourceUseRef, SourceUses};

use crate::CatalogError;
use crate::catalog_read::original::EntryRead;
use crate::starmap_read::{DescriptorInputs, DescriptorPayload};

pub(super) struct DescriptorObservations {
    pub(super) name_inputs_complete: bool,
    pub(super) survey: Option<DescriptorInputs>,
}

impl DescriptorObservations {
    pub(super) fn new(capture_survey: bool) -> Self {
        Self {
            name_inputs_complete: false,
            survey: capture_survey.then(DescriptorInputs::default),
        }
    }

    pub(super) fn observe_payload(
        &mut self,
        read: &EntryRead,
        document: Option<SourceUse>,
    ) -> Result<(), CatalogError> {
        if let Some(inputs) = &mut self.survey {
            inputs.payload = Some(match document {
                Some(document) => DescriptorPayload::Parsed(document),
                None => DescriptorPayload::Attempted(read.receipt().payload_attempt()?),
            });
        }
        Ok(())
    }
}

pub(crate) struct DescriptorSurvey {
    contributors: Vec<SourceUse>,
    attempts: Vec<SourceUseRef>,
    complete: bool,
}

impl DescriptorSurvey {
    pub(super) fn new(directory: Option<SourceUse>) -> Self {
        Self {
            complete: directory.is_some(),
            contributors: directory.into_iter().collect(),
            attempts: Vec::new(),
        }
    }

    pub(super) fn observe(&mut self, inputs: DescriptorInputs) {
        self.complete &= inputs.directory.is_some();
        self.contributors.extend(inputs.directory);
        match inputs.payload {
            Some(DescriptorPayload::Parsed(used)) => self.contributors.push(used),
            Some(DescriptorPayload::Attempted(used)) => {
                self.complete = false;
                self.attempts.push(used.reference().clone());
            }
            None => self.complete = false,
        }
    }

    pub(crate) fn finish(mut self, sources: &SourceUses, routes_complete: bool) -> PlacementSurvey {
        self.contributors
            .sort_unstable_by(|left, right| left.reference().cmp(right.reference()));
        self.contributors
            .dedup_by(|left, right| left.reference() == right.reference());
        self.attempts.sort_unstable();
        self.attempts.dedup();
        PlacementSurvey {
            origin: sources.origin(&self.contributors, routes_complete && self.complete),
            failed_source_uses: self.attempts,
        }
    }
}

/// Original descriptor survey and its attempted reads outside the contributing origin.
#[derive(Debug)]
pub(crate) struct PlacementSurvey {
    pub(crate) origin: FieldOrigin,
    pub(crate) failed_source_uses: Vec<SourceUseRef>,
}

impl PlacementSurvey {
    pub(crate) fn is_complete(&self) -> bool {
        self.origin.source_set_complete() && self.failed_source_uses.is_empty()
    }
}
