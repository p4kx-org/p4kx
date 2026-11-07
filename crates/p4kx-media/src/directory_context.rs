// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Entry, MetadataAccess, MetadataView};

use crate::Source;

/// Metadata access for one already opened, immediately owning Archive source.
#[non_exhaustive]
pub struct DirectoryContext<'source, 'budget, 'context> {
    /// Existing root or physical parent-container identity without a whole-source raw claim.
    pub source: &'context Source,
    /// Source-bound declarations and lazy record cursors.
    pub view: MetadataView<'source>,
    /// Already admitted physical Entries, preserving their original indices.
    pub entries: &'source [Entry],
    /// Shared generation-wide read/work admission and the actual cancellation flag.
    pub access: &'context mut MetadataAccess<'source, 'budget>,
}
