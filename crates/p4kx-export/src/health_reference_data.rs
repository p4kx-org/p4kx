// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Guid;

use crate::HealthText;

/// Descriptive values used for private Reference accounting and final source resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReferenceData {
    pub(crate) field: HealthText,
    pub(crate) source_type: HealthText,
    pub(crate) target_guid: Option<Guid>,
    pub(crate) target_record_index: Option<u32>,
    pub(crate) target_record_path: Option<HealthText>,
    pub(crate) target_type: Option<HealthText>,
    pub(crate) target_location: Option<HealthText>,
    pub(crate) source_pointer: Option<ReferencePointer>,
}

/// Exact retained pointer error parameters without any original-input authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ReferencePointer {
    pub(crate) structure: Option<u32>,
    pub(crate) instance: Option<u32>,
    pub(crate) declared_type: Option<u32>,
}
