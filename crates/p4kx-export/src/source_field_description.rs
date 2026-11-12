// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::FieldError;

#[derive(Debug, Clone, Copy)]
pub(crate) enum FieldIssue {
    Malformed,
    Unreadable,
    Unsupported,
    RetainedVariant,
}

/// Borrowed technical classification, independently of the original body receipt.
pub(crate) struct FieldDescription<'error> {
    pub(crate) issue: FieldIssue,
    pub(crate) source_type: Option<&'error str>,
    pub(crate) field: &'error str,
}

impl FieldError {
    pub(crate) fn description(&self) -> FieldDescription<'_> {
        let mut current = self;
        loop {
            match current {
                Self::Captured(captured) => current = captured.error(),
                Self::Malformed {
                    source_type, field, ..
                } => {
                    return FieldDescription {
                        issue: FieldIssue::Malformed,
                        source_type: Some(source_type),
                        field,
                    };
                }
                Self::Unreadable {
                    source_type, field, ..
                } => {
                    return FieldDescription {
                        issue: FieldIssue::Unreadable,
                        source_type: Some(source_type),
                        field,
                    };
                }
                Self::Unsupported { source_type, field } => {
                    return FieldDescription {
                        issue: FieldIssue::Unsupported,
                        source_type: Some(source_type),
                        field,
                    };
                }
                Self::RetainedVariant { source_type, field } => {
                    return FieldDescription {
                        issue: FieldIssue::RetainedVariant,
                        source_type: Some(source_type),
                        field,
                    };
                }
            }
        }
    }
}
