// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Original animation coordinates shared by extraction reports.

use std::ops::Range;

use p4kx_model::{AnimationChunkCoordinates, AnimationClipCoordinates, AnimationFrameChannel};

use crate::{SourceEvidenceError, SourceLocation, SourceScope, SourceUse, SourceUseRole};

/// Original chunk selectors for one decoded animation input.
#[non_exhaustive]
pub struct AnimationChunkSites {
    /// Header and chunk table selectors.
    pub selectors: Vec<SourceUse>,
    /// Whether the original table row and type prefix were observed.
    pub complete: bool,
}

impl AnimationChunkSites {
    /// Observes the header and table selectors that identify this chunk.
    ///
    /// # Errors
    /// Returns an error if a selector position cannot be represented.
    pub fn of(
        scope: &SourceScope,
        chunk: &AnimationChunkCoordinates,
    ) -> Result<Self, SourceEvidenceError> {
        let mut selectors = Vec::new();
        for (field, start, end) in [
            ("signature", 0, 4),
            ("rowCount", 8, 12),
            ("tableOffset", 12, 16),
        ] {
            selectors.push(site(
                scope,
                "cryEngineChunkHeader",
                field,
                vec![start, end],
                SourceUseRole::Selector,
            )?);
        }
        if let Some(prefix) = chunk.type_prefix() {
            let first = prefix.first_word();
            selectors.push(site(
                scope,
                "cryEngineChunkTable",
                "animationTypePrefix",
                vec![
                    first.start,
                    first.end,
                    prefix.stride(),
                    prefix.count() as u64,
                ],
                SourceUseRole::Selector,
            )?);
        }
        if let Some(row) = chunk.row() {
            for (field, bytes) in [
                ("type", &row.kind),
                ("version", &row.version),
                ("offset", &row.offset),
            ] {
                selectors.push(site(
                    scope,
                    "cryEngineChunkTable",
                    field,
                    vec![chunk.ordinal() as u64, bytes.start, bytes.end],
                    SourceUseRole::Selector,
                )?);
            }
        }
        Ok(Self {
            selectors,
            complete: chunk.row().is_some() && chunk.type_prefix().is_some(),
        })
    }
}

/// Observes one field of the original selected clip.
///
/// # Errors
/// Returns an error if the original position cannot be represented.
pub fn clip_site(
    scope: &SourceScope,
    clip: usize,
    chunk: usize,
    field: &str,
    bytes: Range<u64>,
    role: SourceUseRole,
) -> Result<SourceUse, SourceEvidenceError> {
    site(
        scope,
        "cryEngineAnimation",
        field,
        vec![clip as u64, chunk as u64, bytes.start, bytes.end],
        role,
    )
}

fn frame_site(
    scope: &SourceScope,
    clip: usize,
    chunk: usize,
    controller: usize,
    channel: &str,
    bytes: Range<u64>,
) -> Result<SourceUse, SourceEvidenceError> {
    site(
        scope,
        "cryEngineAnimationFrame",
        channel,
        vec![
            clip as u64,
            chunk as u64,
            controller as u64,
            bytes.start,
            bytes.end,
        ],
        SourceUseRole::Conversion,
    )
}

/// Exact original uses involved in the decoded duration calculation.
#[non_exhaustive]
pub struct AnimationDurationSites {
    /// Timing words and the winning key when it extends the authored duration.
    pub uses: Vec<SourceUse>,
    /// Whether the relevant selectors were observed.
    pub complete: bool,
}

/// Observes the original timing inputs of one decoded clip.
///
/// # Errors
/// Returns an error if an original position cannot be represented.
pub fn duration_sites(
    scope: &SourceScope,
    ordinal: usize,
    coords: &AnimationClipCoordinates,
    authored_end: u32,
    metadata: &AnimationChunkSites,
    data: &AnimationChunkSites,
) -> Result<AnimationDurationSites, SourceEvidenceError> {
    let chunk = coords.metadata_chunk().ordinal();
    let mut uses = metadata.selectors.clone();
    for (field, bytes) in [
        ("frames_per_second", coords.frame_rate()),
        ("end_frame", coords.end_frame()),
    ] {
        uses.push(clip_site(
            scope,
            ordinal,
            chunk,
            field,
            bytes,
            SourceUseRole::Conversion,
        )?);
    }
    let mut complete = metadata.complete;
    if let Some(last) = coords
        .last_frame()
        .filter(|frame| u32::from(frame.frame()) > authored_end)
    {
        uses.extend_from_slice(&data.selectors);
        let channel = match last.channel() {
            AnimationFrameChannel::Rotation => "rotation",
            AnimationFrameChannel::Translation => "translation",
            _ => {
                return Ok(AnimationDurationSites {
                    uses,
                    complete: false,
                });
            }
        };
        let chunk = coords.data_chunk().ordinal();
        uses.push(frame_site(
            scope,
            ordinal,
            chunk,
            last.controller(),
            channel,
            last.bytes(),
        )?);
        uses.push(clip_site(
            scope,
            ordinal,
            chunk,
            "frameChannelDescriptor",
            last.descriptor(),
            SourceUseRole::Selector,
        )?);
        complete &= data.complete;
    }
    Ok(AnimationDurationSites { uses, complete })
}

fn site(
    scope: &SourceScope,
    namespace: &str,
    field: &str,
    indices: Vec<u64>,
    role: SourceUseRole,
) -> Result<SourceUse, SourceEvidenceError> {
    scope.at(
        SourceLocation::Subresource {
            namespace: namespace.into(),
            field: field.into(),
            indices,
        },
        role,
    )
}
