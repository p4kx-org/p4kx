// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::{DataForgeEntry, ObservedTagTree, RecordLocalization};

/// The decoded Record source and its selected exact annotation inputs.
#[derive(Clone, Copy)]
#[non_exhaustive]
pub struct RecordExportSources<'input, 'annotations> {
    /// Entry containing the decoded DataForge.
    pub input: &'input DataForgeEntry,
    /// Indexed language used by the renderer.
    pub localization: Option<&'annotations RecordLocalization<'annotations>>,
    /// Observed tag tree used by the renderer.
    pub tags: Option<&'annotations ObservedTagTree>,
}

impl<'input, 'annotations> RecordExportSources<'input, 'annotations> {
    /// Supplies the original inputs used by the Record writers.
    #[must_use]
    pub const fn new(
        input: &'input DataForgeEntry,
        localization: Option<&'annotations RecordLocalization<'annotations>>,
        tags: Option<&'annotations ObservedTagTree>,
    ) -> Self {
        Self {
            input,
            localization,
            tags,
        }
    }
}
