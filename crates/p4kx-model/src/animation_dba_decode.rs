// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use rayon::prelude::*;

use super::Layout;
use crate::animation_read::{
    AnimationChunk, MAX_ANIMATION_KEYS, PositionCapture, RECORD_LEN, TrackPositions, check_budget,
    declared_keys, decode_tracks, read_record,
};
use crate::{AnimationTiming, CorruptAnimation, Track};

pub(super) struct DecodedClip {
    pub(super) timing: AnimationTiming,
    pub(super) tracks: Vec<Track>,
    pub(super) positions: Option<TrackPositions>,
}

pub(super) fn decode_clips(
    layout: &Layout<'_>,
    data: AnimationChunk<'_>,
    capture: PositionCapture,
) -> Vec<Result<DecodedClip, CorruptAnimation>> {
    layout
        .blocks
        .par_iter()
        .zip(layout.records.par_chunks_exact(RECORD_LEN))
        .map(|(block, record)| {
            let timing = read_record(record)?;
            check_budget(declared_keys(data.bytes, block)?, MAX_ANIMATION_KEYS)?;
            let mut positions = capture.track_positions();
            let tracks = decode_tracks(data, block, &mut positions)?;
            Ok(DecodedClip {
                timing,
                tracks,
                positions,
            })
        })
        .collect()
}
