// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValueState, SourcedValue};

use super::health_text_capture::HealthTextCell;
use crate::{ExportAccountingError, HealthText};

#[derive(Debug, Clone, PartialEq, Eq)]
enum FailurePath {
    Present(HealthText),
    Unreadable,
}

/// Guaranteed human values sharing the sealed occurrence storage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct FailureHuman {
    guid: HealthText,
    path: FailurePath,
    reason: HealthText,
}

impl FailureHuman {
    pub(super) fn new(
        guid: &SourcedValue<HealthText>,
        path: &HealthTextCell,
        reason: &HealthTextCell,
    ) -> Result<Self, ExportAccountingError> {
        let guid = guid
            .value()
            .cloned()
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        let path = match (path.cell().state(), path.value()) {
            (SourceValueState::Present, Some(text)) => FailurePath::Present(text.clone()),
            (SourceValueState::Unreadable, None) => FailurePath::Unreadable,
            _ => return Err(ExportAccountingError::ConflictingSourceDatum),
        };
        let reason = reason
            .value()
            .cloned()
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        Ok(Self { guid, path, reason })
    }

    pub(super) fn guid(&self) -> &str {
        self.guid.as_str()
    }

    pub(super) fn path(&self) -> &str {
        match &self.path {
            FailurePath::Present(text) => text.as_str(),
            FailurePath::Unreadable => "",
        }
    }

    pub(super) fn reason(&self) -> &str {
        self.reason.as_str()
    }
}
