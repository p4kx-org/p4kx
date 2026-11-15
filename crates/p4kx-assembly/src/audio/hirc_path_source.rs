// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;
use std::sync::Arc;

use p4kx_archive::{Archive, ObservedEntry, ObservedEntryBinding};
use p4kx_audio::{ContainerKind, HircKind, HircObject, HircValue};

use super::hirc_diagnostic_sites::{self, DiagnosticSite};

use super::txtp::TxtpApproximation;

use super::walk_banks::Loaded;

/// The original bank and bank-relative byte spans of one resolved HIRC object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HircPathSource {
    entry_position: usize,
    object: Range<usize>,
    id: Range<usize>,
    version: Option<Range<usize>>,
    diagnostics: Option<Arc<BoundDiagnosticSites>>,
    selection_owner: Option<Arc<HircSelectionOwner>>,
    inbound_child: Option<HircSelectionChild>,
}

impl HircPathSource {
    pub(super) fn of(
        archive: &Archive,
        owner: &Loaded<'_>,
        object: &HircObject,
        inbound_child: Option<HircSelectionChild>,
    ) -> Self {
        let entry_position = owner.soundbank.entry_position();
        let version = owner.bank.version_byte_range();
        let selection_owner =
            HircSelectionOwner::of(archive, entry_position, object, version.as_ref()).map(Arc::new);
        Self {
            entry_position,
            object: object.byte_range(),
            id: object.id_byte_range(),
            version,
            diagnostics: BoundDiagnosticSites::of(archive, entry_position, &object.value)
                .map(Arc::new),
            selection_owner,
            inbound_child,
        }
    }

    pub(super) fn selection_owner(&self) -> Option<&HircSelectionOwner> {
        self.selection_owner.as_deref()
    }

    pub(super) const fn inbound_child(&self) -> Option<&HircSelectionChild> {
        self.inbound_child.as_ref()
    }

    pub(super) fn selection_child(
        &self,
        parent: &HircValue,
        ordinal: usize,
    ) -> Option<HircSelectionChild> {
        let owner = self.selection_owner.as_ref()?;
        let bytes = match parent {
            HircValue::Container(container) => container.child_id_byte_range(ordinal),
            HircValue::MusicSwitch(switch) => switch.child_id_byte_range(ordinal),
            _ => None,
        }?;
        Some(HircSelectionChild {
            owner: Arc::clone(owner),
            ordinal,
            bytes,
        })
    }

    pub(super) fn diagnostic_site(
        &self,
        approximation: TxtpApproximation,
    ) -> Option<&DiagnosticSite> {
        self.diagnostics
            .as_ref()?
            .sites
            .iter()
            .find(|site| site.approximation == approximation)
    }

    pub(super) fn diagnostic_entry<'a>(&self, archive: &'a Archive) -> Option<ObservedEntry<'a>> {
        let sites = self.diagnostics.as_ref()?;
        let entry = archive.observed_entry(archive.entries().get(self.entry_position)?)?;
        sites.owner.is_same_entry(&entry.binding()).then_some(entry)
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
struct BoundDiagnosticSites {
    owner: ObservedEntryBinding,
    sites: Vec<DiagnosticSite>,
}

impl BoundDiagnosticSites {
    fn of(archive: &Archive, entry_position: usize, value: &HircValue) -> Option<Self> {
        let sites = hirc_diagnostic_sites::checked_sites(value);
        if sites.is_empty() {
            return None;
        }
        let source = archive.observed_entry(archive.entries().get(entry_position)?)?;
        Some(Self {
            owner: source.binding(),
            sites,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct HircSelectionOwner {
    binding: ObservedEntryBinding,
    entry_position: usize,
    kind: HircKind,
    kind_bytes: Range<u64>,
    random_mode_bytes: Option<Range<u64>>,
    child_count_bytes: Range<u64>,
    version: Option<Range<usize>>,
}

impl HircSelectionOwner {
    fn of(
        archive: &Archive,
        entry_position: usize,
        object: &HircObject,
        version: Option<&Range<usize>>,
    ) -> Option<Self> {
        let (count, mode) = match &object.value {
            HircValue::Container(container) => match container.kind {
                ContainerKind::Random => (
                    container.child_count_evidence()?,
                    Some(container.random_sequence_mode_evidence()?.range.clone()),
                ),
                ContainerKind::Switch => (container.child_count_evidence()?, None),
                _ => return None,
            },
            HircValue::MusicSwitch(switch) => (switch.child_count_evidence()?, None),
            _ => return None,
        };
        let source = archive.observed_entry(archive.entries().get(entry_position)?)?;
        let kind = object.kind_byte_range();
        Some(Self {
            binding: source.binding(),
            entry_position,
            kind: object.kind,
            kind_bytes: kind.start as u64..kind.end as u64,
            random_mode_bytes: mode,
            child_count_bytes: count.range.clone(),
            version: version.cloned(),
        })
    }

    pub(super) fn entry<'a>(&self, archive: &'a Archive) -> Option<ObservedEntry<'a>> {
        let entry = archive.observed_entry(archive.entries().get(self.entry_position)?)?;
        self.binding
            .is_same_entry(&entry.binding())
            .then_some(entry)
    }

    pub(super) const fn kind(&self) -> HircKind {
        self.kind
    }

    pub(super) fn kind_bytes(&self) -> Range<u64> {
        self.kind_bytes.clone()
    }

    pub(super) fn random_mode_bytes(&self) -> Option<Range<u64>> {
        self.random_mode_bytes.clone()
    }

    pub(super) fn child_count_bytes(&self) -> Range<u64> {
        self.child_count_bytes.clone()
    }

    pub(super) fn version_bytes(&self) -> Option<Range<usize>> {
        self.version.clone()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct HircSelectionChild {
    owner: Arc<HircSelectionOwner>,
    ordinal: usize,
    bytes: Range<u64>,
}

impl HircSelectionChild {
    pub(super) fn owner(&self) -> &HircSelectionOwner {
        &self.owner
    }

    pub(super) const fn ordinal(&self) -> usize {
        self.ordinal
    }

    pub(super) fn id_bytes(&self) -> Range<u64> {
        self.bytes.clone()
    }
}

#[derive(Clone, Copy)]
pub(super) enum PathSourceMode {
    Omit,
    Retain,
}
