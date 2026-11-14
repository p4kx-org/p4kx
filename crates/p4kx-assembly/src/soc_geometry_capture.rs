// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::ops::Range;
use std::sync::Arc;

use p4kx_extraction::{EntryOrigin, ObservedEntryBinding, PathIndex};
use p4kx_model::{IncludedPathSlot, SocEntityFile};

use crate::interior;
use crate::provider;
use crate::soc_container_capture::soc_archive_member::{self, SocArchiveMember};
use crate::soc_container_capture::soc_descriptor_member::SocMemberSelection;
use crate::soc_container_capture::{CaptureCheck, SocCaptureError, SocContainerOccurrence};

/// One original model path and its physical lookup for a logical SOC stream occurrence.
#[derive(Debug)]
#[non_exhaustive]
pub struct SocGeometryReference {
    /// Original SOC chunk table index.
    pub chunk_ordinal: usize,
    /// Original slot in the chunk's model path table.
    pub model_ordinal: usize,
    /// File-relative byte interval of the model path slot in the decoded SOC member.
    pub source_range: Range<u64>,
    /// Physical path lookup at the winning directory tier, when one exists.
    pub target: SocGeometryTarget,
}

/// The first owner and path form whose directory contains matching Entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct SocGeometryTier {
    /// Distance from the SOC member's container, with zero for the immediate container.
    pub depth: usize,
    /// Which spelling from the model provider's path candidates matched.
    pub kind: SocGeometryCandidateKind,
}

/// One canonical model path candidate spelling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SocGeometryCandidateKind {
    /// The relative model path with the Data prefix.
    DataPrefixed,
    /// The relative model path without adding a prefix.
    Relative,
}

/// The outcome of directory lookup for an original SOC model path slot.
#[derive(Debug)]
#[non_exhaustive]
pub enum SocGeometryTarget {
    /// The original path bytes do not form a usable lookup selector.
    InvalidSelector,
    /// No searchable owner has a matching selector.
    Missing,
    /// One physical directory Entry matches in the winning tier.
    Matched {
        /// The matching owner distance and path form.
        tier: SocGeometryTier,
        /// The exact Entry and its directory ancestry.
        entry: EntryOrigin,
    },
    /// Multiple distinct physical Entries match in the winning tier.
    Ambiguous {
        /// The matching owner distance and path form.
        tier: SocGeometryTier,
        /// All matching Entries in their physical order.
        entries: Vec<EntryOrigin>,
    },
}

pub(super) fn capture<'archive>(
    occurrence: &mut SocContainerOccurrence,
    layers: impl Iterator<Item = PathIndex<'archive>>,
    check: CaptureCheck<'_>,
) -> Result<(), SocCaptureError> {
    let layers: Vec<_> = layers.collect();
    let mut cached: HashMap<ObservedEntryBinding, Arc<[SocGeometryReference]>> = HashMap::new();
    for stream in &mut occurrence.streams {
        check.poll()?;
        let Some(member) = stream
            .member
            .as_ref()
            .and_then(SocMemberSelection::selected)
        else {
            continue;
        };
        let binding = member.origin().binding();
        stream.geometry = if let Some(known) = cached.get(&binding) {
            Arc::clone(known)
        } else {
            let rows: Arc<[_]> = scan_member(member, &layers, check)?.into();
            cached.try_reserve(1)?;
            cached.insert(binding, Arc::clone(&rows));
            rows
        };
    }
    Ok(())
}

fn scan_member(
    member: &SocArchiveMember,
    layers: &[PathIndex<'_>],
    check: CaptureCheck<'_>,
) -> Result<Vec<SocGeometryReference>, SocCaptureError> {
    let Some(file) = member
        .payload()
        .ok()
        .and_then(|bytes| SocEntityFile::parse(bytes).ok())
    else {
        return Ok(Vec::new());
    };
    let mut result = Vec::new();
    for chunk in file.included_objects_chunks() {
        check.poll()?;
        let Ok(tables) = chunk.tables() else {
            continue;
        };
        let slots = tables.models();
        result.try_reserve(slots.len())?;
        for slot in slots {
            check.poll()?;
            let offset = chunk.header().offset();
            let range = slot.range();
            result.push(SocGeometryReference {
                chunk_ordinal: chunk.chunk_index(),
                model_ordinal: slot.ordinal(),
                source_range: offset + range.start..offset + range.end,
                target: resolve(slot, layers, check)?,
            });
        }
    }
    Ok(result)
}

fn resolve(
    slot: IncludedPathSlot<'_>,
    layers: &[PathIndex<'_>],
    check: CaptureCheck<'_>,
) -> Result<SocGeometryTarget, SocCaptureError> {
    let Some(path) = std::str::from_utf8(slot.path_bytes())
        .ok()
        .filter(|_| slot.bytes().contains(&0))
        .map(str::trim)
        .filter(|path| !path.is_empty() && interior::plain_container_path(path))
    else {
        return Ok(SocGeometryTarget::InvalidSelector);
    };
    let [prefixed, relative] = provider::archive_entry_candidates(path);
    for (depth, index) in layers.iter().enumerate() {
        for (kind, path) in [
            (SocGeometryCandidateKind::DataPrefixed, prefixed.as_str()),
            (SocGeometryCandidateKind::Relative, relative.as_str()),
        ] {
            let matches = soc_archive_member::select(index, [path], || check.poll())?;
            let tier = SocGeometryTier { depth, kind };
            match matches.as_slice() {
                [] => {}
                [entry] => {
                    return Ok(SocGeometryTarget::Matched {
                        tier,
                        entry: soc_archive_member::origin(index.archive(), entry)?,
                    });
                }
                entries => {
                    let entries = entries
                        .iter()
                        .map(|entry| soc_archive_member::origin(index.archive(), entry))
                        .collect::<Result<_, _>>()?;
                    return Ok(SocGeometryTarget::Ambiguous { tier, entries });
                }
            }
        }
    }
    Ok(SocGeometryTarget::Missing)
}
