// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

/// A genuinely unavailable contextual lookup, separate from missing source declarations.
#[derive(Debug, thiserror::Error)]
pub(crate) enum TemplateContextError {
    #[error("the original Tag inputs do not establish {source_type}.{field}")]
    TagLookupUnavailable { field: String, source_type: String },
}
