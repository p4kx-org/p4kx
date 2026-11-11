// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::Value;

use crate::audio_bank_music_playlist::{self, MusicPlaylistPayload};
use crate::audio_bank_music_segment::{self, MusicSegmentPayload};
use crate::audio_bank_music_stinger_schedule_payload::{self, MusicStingerSchedulePayload};
use crate::audio_bank_music_switch::{self, MusicSwitchPayload};
use crate::audio_bank_music_track::{self, MusicTrackPayload};
use crate::audio_schema_status::StatusRule;
use crate::{MediaError, schema_merge};

use super::Binding;

const MUSIC_PLAYLIST_PAYLOAD: &str = "MusicPlaylistPayload";
const MUSIC_SEGMENT_PAYLOAD: &str = "MusicSegmentPayload";
const MUSIC_SWITCH_PAYLOAD: &str = "MusicSwitchPayload";
const MUSIC_TRACK_PAYLOAD: &str = "MusicTrackPayload";
const STINGER_SCHEDULE_PAYLOAD: &str = "MusicStingerSchedulePayload";

pub(super) fn insert_payload(base: &mut Value) -> Result<(), MediaError> {
    schema_merge::insert_payload(
        base,
        MUSIC_SEGMENT_PAYLOAD,
        schema_merge::serialized_payload::<MusicSegmentPayload<'static>>(),
    )?;
    schema_merge::insert_payload(
        base,
        MUSIC_TRACK_PAYLOAD,
        schema_merge::serialized_payload::<MusicTrackPayload>(),
    )?;
    schema_merge::insert_payload(
        base,
        MUSIC_PLAYLIST_PAYLOAD,
        schema_merge::serialized_payload::<MusicPlaylistPayload>(),
    )?;
    schema_merge::insert_payload(
        base,
        MUSIC_SWITCH_PAYLOAD,
        schema_merge::serialized_payload::<MusicSwitchPayload>(),
    )?;
    schema_merge::insert_payload(
        base,
        STINGER_SCHEDULE_PAYLOAD,
        schema_merge::serialized_payload::<MusicStingerSchedulePayload<'static>>(),
    )
}

pub(super) const fn bindings() -> [Binding; 19] {
    let [segment, track_id, segment_stinger, marker] = segment_bindings();
    let [track, source, clip, switch_member] = track_bindings();
    let [playlist, child, playlist_stinger, item] = playlist_bindings();
    let [switch, switch_child, switch_stinger, group, node, target] = switch_bindings();
    [
        segment,
        track_id,
        segment_stinger,
        marker,
        track,
        source,
        clip,
        switch_member,
        playlist,
        child,
        playlist_stinger,
        item,
        switch,
        switch_child,
        switch_stinger,
        group,
        node,
        target,
        Binding::exact(
            STINGER_SCHEDULE_PAYLOAD,
            audio_bank_music_stinger_schedule_payload::NAMESPACE,
            "bank-music-stinger-schedule",
            3,
            StatusRule::MalformedLayout,
        ),
    ]
}

const fn switch_bindings() -> [Binding; 6] {
    [
        Binding::exact(
            MUSIC_SWITCH_PAYLOAD,
            audio_bank_music_switch::NAMESPACE,
            "bank-music-switch",
            2,
            StatusRule::MalformedLayout,
        ),
        Binding::exact(
            MUSIC_SWITCH_PAYLOAD,
            audio_bank_music_switch::CHILD,
            "bank-music-switch-child",
            3,
            StatusRule::Complete,
        ),
        Binding::exact(
            MUSIC_SWITCH_PAYLOAD,
            audio_bank_music_switch::STINGER,
            "bank-music-switch-stinger",
            3,
            StatusRule::Complete,
        ),
        Binding::exact(
            MUSIC_SWITCH_PAYLOAD,
            audio_bank_music_switch::GROUP,
            "bank-music-switch-group",
            3,
            StatusRule::Complete,
        ),
        Binding::exact(
            MUSIC_SWITCH_PAYLOAD,
            audio_bank_music_switch::NODE,
            "bank-music-switch-node",
            3,
            StatusRule::Complete,
        ),
        Binding::exact(
            MUSIC_SWITCH_PAYLOAD,
            audio_bank_music_switch::TARGET,
            "bank-music-switch-target",
            3,
            StatusRule::Complete,
        ),
    ]
}

const fn segment_bindings() -> [Binding; 4] {
    [
        Binding::exact(
            MUSIC_SEGMENT_PAYLOAD,
            audio_bank_music_segment::NAMESPACE,
            "bank-music-segment",
            2,
            StatusRule::MalformedLayout,
        ),
        Binding::exact(
            MUSIC_SEGMENT_PAYLOAD,
            audio_bank_music_segment::CHILD,
            "bank-music-segment-child",
            3,
            StatusRule::Complete,
        ),
        Binding::exact(
            MUSIC_SEGMENT_PAYLOAD,
            audio_bank_music_segment::STINGER,
            "bank-music-segment-stinger",
            3,
            StatusRule::Complete,
        ),
        Binding::exact(
            MUSIC_SEGMENT_PAYLOAD,
            audio_bank_music_segment::MARKER,
            "bank-music-segment-marker",
            3,
            StatusRule::Complete,
        ),
    ]
}

const fn track_bindings() -> [Binding; 4] {
    [
        Binding::exact(
            MUSIC_TRACK_PAYLOAD,
            audio_bank_music_track::NAMESPACE,
            "bank-music-track",
            2,
            StatusRule::MalformedLayout,
        ),
        Binding::exact(
            MUSIC_TRACK_PAYLOAD,
            audio_bank_music_track::SOURCE,
            "bank-music-track-source",
            3,
            StatusRule::Complete,
        ),
        Binding::exact(
            MUSIC_TRACK_PAYLOAD,
            audio_bank_music_track::CLIP,
            "bank-music-track-clip",
            3,
            StatusRule::Complete,
        ),
        Binding::exact(
            MUSIC_TRACK_PAYLOAD,
            audio_bank_music_track::SWITCH_MEMBER,
            "bank-music-track-switch-member",
            3,
            StatusRule::Complete,
        ),
    ]
}

const fn playlist_bindings() -> [Binding; 4] {
    [
        Binding::exact(
            MUSIC_PLAYLIST_PAYLOAD,
            audio_bank_music_playlist::NAMESPACE,
            "bank-music-playlist",
            2,
            StatusRule::MalformedLayout,
        ),
        Binding::exact(
            MUSIC_PLAYLIST_PAYLOAD,
            audio_bank_music_playlist::CHILD,
            "bank-music-playlist-child",
            3,
            StatusRule::Complete,
        ),
        Binding::exact(
            MUSIC_PLAYLIST_PAYLOAD,
            audio_bank_music_playlist::STINGER,
            "bank-music-playlist-stinger",
            3,
            StatusRule::Complete,
        ),
        Binding::exact(
            MUSIC_PLAYLIST_PAYLOAD,
            audio_bank_music_playlist::ITEM,
            "bank-music-playlist-item",
            3,
            StatusRule::Complete,
        ),
    ]
}
