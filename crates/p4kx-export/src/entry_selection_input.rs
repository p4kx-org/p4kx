// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_extraction::{EntryOrigin, SourceLocation, SourceUseRole};

use super::health_reference_receipt::ReferenceFrame;

#[derive(Debug, Clone)]
pub(super) struct EntrySelectionUse {
    pub(super) source: SelectionSource,
    pub(super) location: SourceLocation,
    pub(super) role: SourceUseRole,
}

#[derive(Debug, Clone)]
pub(super) enum SelectionSource {
    Archive(EntryOrigin),
    Entry(super::health_entry_metadata::EntryMetadata),
    Record(Arc<ReferenceFrame>),
    UnboundLoadout(p4kx_assembly::LoadoutFileObservation),
}
