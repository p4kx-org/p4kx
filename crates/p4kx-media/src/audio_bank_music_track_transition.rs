// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::bank::{BankMusicTrackFadeFields, BankMusicTrackTransitionFields};

use crate::MediaError;
use crate::audio_bank_music_track_control_origins;
use crate::audio_bank_music_track_control_payload::{TrackFade, TrackTransition};
use crate::audio_bank_node_origins::NodeSites;
use crate::audio_bank_origins::BankOrigins;

pub(crate) fn observe(
    sites: &NodeSites,
    prepared: &mut BankOrigins<'_, '_, '_>,
    original: &BankMusicTrackTransitionFields,
) -> Result<TrackTransition, MediaError> {
    Ok(TrackTransition {
        source: fade(sites, prepared, &original.source, Side::Source)?,
        sync_type: sites.scalar(
            prepared,
            "observed/transition/sync_type",
            original.sync_type.as_ref(),
            "musicTrackTransitionSyncType",
        )?,
        cue_filter_hash: sites.scalar(
            prepared,
            "observed/transition/cue_filter_hash",
            original.cue_filter_hash.as_ref(),
            "musicTrackTransitionCueFilterHash",
        )?,
        destination: fade(sites, prepared, &original.destination, Side::Destination)?,
    })
}

#[derive(Clone, Copy)]
enum Side {
    Source,
    Destination,
}

fn fade(
    sites: &NodeSites,
    prepared: &mut BankOrigins<'_, '_, '_>,
    original: &BankMusicTrackFadeFields,
    side: Side,
) -> Result<Option<TrackFade>, MediaError> {
    if original.time_bits.is_none() {
        return Ok(None);
    }
    let (path, time, curve, offset) = match side {
        Side::Source => (
            "observed/transition/source",
            "musicTrackSourceTransitionTimeBits",
            "musicTrackSourceFadeCurve",
            "musicTrackSourceTransitionOffsetBits",
        ),
        Side::Destination => (
            "observed/transition/destination",
            "musicTrackDestinationTransitionTimeBits",
            "musicTrackDestinationFadeCurve",
            "musicTrackDestinationTransitionOffsetBits",
        ),
    };
    Ok(Some(TrackFade {
        time: audio_bank_music_track_control_origins::signed(
            sites,
            prepared,
            original.time_bits.as_ref(),
            &format!("{path}/time"),
            time,
        )?,
        curve: sites.scalar(
            prepared,
            &format!("{path}/curve"),
            original.curve.as_ref(),
            curve,
        )?,
        offset: audio_bank_music_track_control_origins::signed(
            sites,
            prepared,
            original.offset_bits.as_ref(),
            &format!("{path}/offset"),
            offset,
        )?,
    }))
}
