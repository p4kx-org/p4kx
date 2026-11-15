// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_archive::{Archive, ObservedEntry, ObservedEntryBinding};
use p4kx_audio::{HircObject, HircValue};

use super::txtp::TxtpApproximation;

use super::walk_banks::Loaded;

/// The original bank and bank-relative byte spans of one resolved HIRC object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HircPathSource {
    entry_position: usize,
    object: Range<usize>,
    id: Range<usize>,
    version: Option<Range<usize>>,
    diagnostic: Option<DiagnosticSite>,
}

impl HircPathSource {
    pub(super) fn of(archive: &Archive, owner: &Loaded<'_>, object: &HircObject) -> Self {
        let entry_position = owner.soundbank.entry_position();
        Self {
            entry_position,
            object: object.byte_range(),
            id: object.id_byte_range(),
            version: owner.bank.version_byte_range(),
            diagnostic: DiagnosticSite::of(archive, entry_position, &object.value),
        }
    }

    pub(super) fn diagnostic_bytes(&self, approximation: TxtpApproximation) -> Option<Range<u64>> {
        let site = self.diagnostic.as_ref()?;
        (site.approximation == approximation).then(|| site.bytes.clone())
    }

    pub(super) fn diagnostic_entry<'a>(&self, archive: &'a Archive) -> Option<ObservedEntry<'a>> {
        let site = self.diagnostic.as_ref()?;
        let entry = archive.observed_entry(archive.entries().get(self.entry_position)?)?;
        site.owner.is_same_entry(&entry.binding()).then_some(entry)
    }

    /// Returns the bank's physical Entry position in the Archive.
    #[must_use]
    pub const fn entry_position(&self) -> usize {
        self.entry_position
    }

    /// Returns the half-open extent of the object within the decoded bank bytes.
    #[must_use]
    pub fn object_bytes(&self) -> Range<usize> {
        self.object.clone()
    }

    /// Returns the half-open extent of the object ID.
    #[must_use]
    pub fn id_bytes(&self) -> Range<usize> {
        self.id.clone()
    }

    /// Returns the half-open extent of the bank-version selector.
    #[must_use]
    pub fn version_bytes(&self) -> Option<Range<usize>> {
        self.version.clone()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DiagnosticSite {
    approximation: TxtpApproximation,
    bytes: Range<u64>,
    owner: ObservedEntryBinding,
}

impl DiagnosticSite {
    fn of(archive: &Archive, entry_position: usize, value: &HircValue) -> Option<Self> {
        let HircValue::Container(container) = value else {
            return None;
        };
        let (approximation, bytes) = if let Some(mode) = container.transition_mode_evidence() {
            (TxtpApproximation::OmitContainerTransitions, &mode.range)
        } else {
            let points = container.blend_point_count_evidence()?;
            (TxtpApproximation::OmitBlendCurves, &points.range)
        };
        let source = archive.observed_entry(archive.entries().get(entry_position)?)?;
        Some(Self {
            approximation,
            bytes: bytes.clone(),
            owner: source.binding(),
        })
    }
}

#[derive(Clone, Copy)]
pub(super) enum PathSourceMode {
    Omit,
    Retain,
}
