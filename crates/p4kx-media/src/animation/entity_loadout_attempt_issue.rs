// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{
    LoadoutDocumentKind, LoadoutFileError, LoadoutPath, LoadoutReadEvent, LoadoutRequest,
};
use p4kx_extraction::{EntryOrigin, InputScope, SourceUseRole};

use crate::origin_capture::{OriginCapture, OriginSite};
use crate::origin_contributions::OriginContributions;
use crate::{MediaError, MediaIssue, MediaWriter, Severity};

pub(super) struct AttemptIssue {
    pub(super) request: LoadoutPath,
    original: Option<EntryOrigin>,
    code: &'static str,
    reason: String,
}

impl AttemptIssue {
    pub(super) fn from_event(event: &LoadoutReadEvent<'_, '_>) -> Option<Self> {
        match event {
            LoadoutReadEvent::FileRead {
                observation,
                request,
                ..
            } if matches!(observation.kind(), LoadoutDocumentKind::XmlLoadout) => {
                let error = observation.read_outcome().err()?;
                let code = match error {
                    LoadoutFileError::NotFound => "animation.entity.loadout.missing",
                    LoadoutFileError::Ambiguous => "animation.entity.loadout.ambiguous",
                    LoadoutFileError::NotSelected => "animation.entity.loadout.not-selected",
                    LoadoutFileError::TooLarge { .. } => "animation.entity.loadout.too-large",
                    LoadoutFileError::UnprovedSelection => {
                        "animation.entity.loadout.unproved-selection"
                    }
                    _ => "animation.entity.loadout.unreadable",
                };
                Some(Self {
                    request: (*request).clone(),
                    original: None,
                    code,
                    reason: error.to_string(),
                })
            }
            LoadoutReadEvent::FileProjection {
                observation,
                request,
                ..
            } if matches!(observation.kind(), LoadoutDocumentKind::XmlLoadout) => {
                let reason = observation.projection_outcome()?.err()?;
                Some(Self {
                    request: (*request).clone(),
                    original: observation.source().entry_origin().cloned(),
                    code: "animation.entity.loadout.malformed",
                    reason: reason.to_owned(),
                })
            }
            LoadoutReadEvent::FileDependencyRefused {
                dependency, error, ..
            } => {
                let LoadoutRequest::Xml { path } = dependency else {
                    return None;
                };
                Some(Self {
                    request: path.clone(),
                    original: None,
                    code: "animation.entity.loadout.include-refused",
                    reason: error.to_string(),
                })
            }
            _ => None,
        }
    }

    pub(super) fn issue(&self) -> MediaIssue {
        MediaIssue::new(
            self.code,
            "loadout",
            Severity::Warning,
            format!("{}: {}", self.request, self.reason),
        )
    }

    pub(super) fn add_original(
        &self,
        capture: &mut OriginCapture,
        writer: &MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let Some(original) = &self.original else {
            return capture.include_original(&OriginContributions::unknown(&writer.origins.owner));
        };
        let input = writer.origins.observer()?.original(original)?;
        capture.add(
            &input,
            InputScope::Payload,
            OriginSite::new("media-physical-input", &[], "complete-payload"),
            SourceUseRole::Conversion,
            writer,
        )
    }
}
