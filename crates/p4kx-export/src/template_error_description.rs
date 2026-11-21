// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::TemplateError;

/// Borrowed Domain classification without the original receipt transport wrapper.
pub(crate) enum TemplateDescription<'error> {
    UnknownStruct {
        field: &'static str,
        name: &'error str,
    },
}

impl TemplateError {
    pub(crate) fn description(&self) -> TemplateDescription<'_> {
        let mut current = self;
        loop {
            match current {
                Self::Captured { error, .. } => current = error,
                Self::UnknownStruct { field, name } => {
                    return TemplateDescription::UnknownStruct { field, name };
                }
            }
        }
    }
}
