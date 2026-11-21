// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;
use std::sync::Arc;

use p4kx_archive::{Archive, ObservedEntry, ObservedEntryBinding};
use p4kx_audio::{ContainerKind, HircKind, HircObject, HircValue};

use super::hirc_diagnostic_sites::{self, DiagnosticSite};
pub(super) use super::hirc_route_source::HircRouteSource;

use super::terminal_media_origin::TerminalSourceTable;
use super::txtp::TxtpApproximation;

use super::walk_banks::Loaded;

#[path = "hirc_path_origins.rs"]
mod origins;

pub use self::origins::HircPathOrigins;

/// The original bank and bank-relative byte spans of one resolved HIRC object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HircPathSource {
    entry_position: usize,
    binding: Option<ObservedEntryBinding>,
    object: Range<usize>,
    id: Range<usize>,
    kind_code: Range<usize>,
    kind_evidence: NodeKindEvidence,
    version: Option<Range<usize>>,
    diagnostics: Option<Arc<[DiagnosticSite]>>,
    child_owner: Option<Arc<HircChildOwner>>,
    inbound: HircArrival,
    pub(super) terminal: Option<Arc<TerminalSourceTable>>,
}

impl HircPathSource {
    pub(super) fn of(
        archive: &Archive,
        owner: &Loaded<'_>,
        object: &HircObject,
        inbound: HircArrival,
        inbound_diagnostic: Option<DiagnosticSite>,
    ) -> Self {
        let entry_position = owner.soundbank.entry_position();
        let binding = bind_entry(archive, entry_position);
        let version = owner.bank.version_byte_range();
        let child_owner = binding
            .as_ref()
            .and_then(|binding| {
                HircChildOwner::of(binding, entry_position, object, version.as_ref())
            })
            .map(Arc::new);
        let mut diagnostics = hirc_diagnostic_sites::checked_sites(&object.value);
        diagnostics.extend(inbound_diagnostic);
        Self {
            entry_position,
            binding,
            object: object.byte_range(),
            id: object.id_byte_range(),
            kind_code: object.kind_byte_range(),
            kind_evidence: NodeKindEvidence::of(&object.value),
            version,
            diagnostics: (!diagnostics.is_empty()).then(|| diagnostics.into()),
            child_owner,
            inbound,
            terminal: TerminalSourceTable::capture(archive, owner, object).map(Arc::new),
        }
    }

    pub(super) const fn inbound_child(&self) -> Option<&HircChildSource> {
        match &self.inbound {
            HircArrival::Link(Some(HircInboundSource::Child(child))) => Some(child),
            _ => None,
        }
    }

    pub(super) fn inbound_route(&self) -> Option<&HircRouteSource> {
        match &self.inbound {
            HircArrival::Link(Some(
                HircInboundSource::Route(route) | HircInboundSource::Stinger { action: route, .. },
            )) => Some(route),
            _ => None,
        }
    }

    pub(super) fn child(&self, parent: &HircValue, ordinal: usize) -> Option<HircChildSource> {
        let owner = self.child_owner.as_ref()?;
        let bytes = match parent {
            HircValue::Container(container) => container.child_id_byte_range(ordinal),
            HircValue::MusicSegment(segment) => segment.node.child_id_byte_range(ordinal),
            HircValue::MusicSwitch(switch) => switch.node.child_id_byte_range(ordinal),
            HircValue::MusicPlaylist(playlist) => playlist.node.child_id_byte_range(ordinal),
            _ => None,
        }?;
        Some(HircChildSource {
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
            .as_deref()?
            .iter()
            .find(|site| site.approximation == approximation)
    }

    /// Returns the exact observed soundbank Entry that held this object.
    #[must_use]
    pub fn entry<'a>(&self, archive: &'a Archive) -> Option<ObservedEntry<'a>> {
        let entry = archive.observed_entry(archive.entries().get(self.entry_position)?)?;
        self.binding
            .as_ref()?
            .is_same_entry(&entry.binding())
            .then_some(entry)
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
pub(super) enum HircInboundSource {
    Child(HircChildSource),
    Route(Arc<HircRouteSource>),
    Stinger {
        action: Arc<HircRouteSource>,
        schedule: Option<Arc<HircRouteSource>>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum HircArrival {
    Start,
    Link(Option<HircInboundSource>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum NodeKindEvidence {
    TypeCode,
    RandomSequence(Option<Range<u64>>),
}

impl NodeKindEvidence {
    fn of(value: &HircValue) -> Self {
        match value {
            HircValue::Container(container)
                if matches!(
                    container.kind,
                    ContainerKind::Random | ContainerKind::Sequence
                ) =>
            {
                Self::RandomSequence(
                    container
                        .random_sequence_mode_evidence()
                        .map(|word| word.range.clone()),
                )
            }
            _ => Self::TypeCode,
        }
    }
}

pub(super) fn bind_entry(archive: &Archive, entry_position: usize) -> Option<ObservedEntryBinding> {
    let entry = archive.entries().get(entry_position)?;
    Some(archive.observed_entry(entry)?.binding())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct HircChildOwner {
    binding: ObservedEntryBinding,
    entry_position: usize,
    kind: HircKind,
    kind_bytes: Range<u64>,
    random_mode_bytes: Option<Range<u64>>,
    child_count_bytes: Range<u64>,
    version: Option<Range<usize>>,
}

impl HircChildOwner {
    fn of(
        binding: &ObservedEntryBinding,
        entry_position: usize,
        object: &HircObject,
        version: Option<&Range<usize>>,
    ) -> Option<Self> {
        let (count, mode) = match &object.value {
            HircValue::Container(container) => {
                let mode = match container.kind {
                    ContainerKind::Random | ContainerKind::Sequence => {
                        Some(container.random_sequence_mode_evidence()?.range.clone())
                    }
                    ContainerKind::Switch | ContainerKind::Blend => None,
                    _ => return None,
                };
                (container.child_count_evidence()?, mode)
            }
            HircValue::MusicSegment(segment) => (segment.node.child_count_evidence()?, None),
            HircValue::MusicSwitch(switch) => (switch.node.child_count_evidence()?, None),
            HircValue::MusicPlaylist(playlist) => (playlist.node.child_count_evidence()?, None),
            _ => return None,
        };
        let kind = object.kind_byte_range();
        Some(Self {
            binding: binding.clone(),
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

    pub(super) const fn child_fields(&self) -> Option<HircChildFields> {
        let fields = match self.kind {
            HircKind::RandomSequenceContainer => {
                HircChildFields::container("bnkHircRandomSequence")
            }
            HircKind::SwitchContainer => HircChildFields::container("bnkHircSwitchContainer"),
            HircKind::LayerContainer => HircChildFields::container("bnkHircBlendContainer"),
            HircKind::MusicSegment | HircKind::MusicSwitch | HircKind::MusicPlaylist => {
                HircChildFields {
                    namespace: "bnkHircMusicNode",
                    count: "musicChildCount",
                    id: "musicChildId",
                }
            }
            _ => return None,
        };
        Some(fields)
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
pub(super) struct HircChildSource {
    owner: Arc<HircChildOwner>,
    ordinal: usize,
    bytes: Range<u64>,
}

impl HircChildSource {
    pub(super) fn owner(&self) -> &HircChildOwner {
        &self.owner
    }

    pub(super) const fn ordinal(&self) -> usize {
        self.ordinal
    }

    pub(super) fn id_bytes(&self) -> Range<u64> {
        self.bytes.clone()
    }
}

pub(super) struct HircChildFields {
    pub(super) namespace: &'static str,
    pub(super) count: &'static str,
    pub(super) id: &'static str,
}

impl HircChildFields {
    const fn container(namespace: &'static str) -> Self {
        Self {
            namespace,
            count: "containerChildCount",
            id: "containerChildId",
        }
    }
}

#[derive(Clone, Copy)]
pub(super) enum PathSourceMode {
    Omit,
    Retain,
}

impl PathSourceMode {
    pub(super) fn entry_binding(
        self,
        archive: &Archive,
        position: usize,
    ) -> Option<ObservedEntryBinding> {
        match self {
            Self::Omit => None,
            Self::Retain => bind_entry(archive, position),
        }
    }
}
