// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

/// Relationship counts within the catalog's first-eligible nominal-GUID quantization policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct QuantizationSummary {
    /// Selected inverse-eligible quantizations with no assigned resource GUIDs.
    pub unused: usize,
    /// Deduplicated resource links without an eligible target, plus selected leftover links.
    /// A Reference without a resolved physical Record contributes no link.
    pub unresolved_links: usize,
}
