// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Builds the type-dependent part of an [`ObjectView`](crate::hirc_view::ObjectView) from what the
//! reader decodes of a `HIRC` object.

use p4kx_audio::{
    ContainerKind, HircValue, MediaStorage, MusicClip, MusicMarker, MusicPlaylist, MusicSegment,
    MusicStinger, MusicSwitch, MusicTrack, MusicTrackKind, MusicTrackSwitch, SoundSource,
};

use super::state as hirc_view_state;
use crate::hirc_view::{
    ActionDetail, ClipView, ContainerDetail, Detail, ItemView, MarkerView, PlaylistDetail,
    SegmentDetail, SoundDetail, StingerView, SwitchDetail, TrackDetail, TrackSwitchView,
};

/// Returns the type-dependent fields of a decoded object; `None` for an Event, a raw body and a
/// malformed one.
pub(crate) fn detail(value: &HircValue) -> Option<Detail<'_>> {
    Some(match value {
        HircValue::State(state) => Detail::State(hirc_view_state::detail(state)),
        HircValue::Sound(source) => Detail::Sound(sound(source)),
        HircValue::Action(action) => Detail::Action(ActionDetail {
            action_type: action.action_type,
            target: action.target_id,
            soundbank_id: action.bank.map(|bank| bank.bank_id),
        }),
        HircValue::Container(container) => Detail::Container(ContainerDetail {
            container_kind: container_kind(container.kind),
        }),
        HircValue::MusicSegment(segment) => Detail::Segment(segment_detail(segment)),
        HircValue::MusicTrack(track) => Detail::Track(track_detail(track)),
        HircValue::MusicSwitch(switch) => Detail::Switch(switch_detail(switch)),
        HircValue::MusicPlaylist(playlist) => Detail::Playlist(playlist_detail(playlist)),
        HircValue::Event(_) | HircValue::Raw | HircValue::Malformed(_) => return None,
    })
}

const fn sound(source: &SoundSource) -> SoundDetail {
    SoundDetail {
        plugin_id: source.plugin_id,
        storage: match source.storage {
            MediaStorage::Embedded => "embedded",
            MediaStorage::PrefetchStreamed => "prefetch-streamed",
            MediaStorage::Streamed => "streamed",
            MediaStorage::Other(_) => "other",
        },
        source_id: source.source_id,
        in_memory_size: source.in_memory_size,
        language_specific: source.is_language_specific(),
        source_plugin: source.is_source_plugin(),
    }
}

const fn container_kind(kind: ContainerKind) -> &'static str {
    match kind {
        ContainerKind::Random => "random",
        ContainerKind::Sequence => "sequence",
        ContainerKind::Switch => "switch",
        ContainerKind::Blend => "blend",
        _ => "other",
    }
}

fn segment_detail(segment: &MusicSegment) -> SegmentDetail<'_> {
    let meter = &segment.node.meter;
    SegmentDetail {
        duration_ms: segment.duration,
        tempo: meter.tempo,
        beats_per_bar: meter.beats_per_bar,
        beat_value: meter.beat_value,
        markers: segment.markers.iter().map(marker).collect(),
        stingers: segment.node.stingers.iter().copied().map(stinger).collect(),
    }
}

fn marker(marker: &MusicMarker) -> MarkerView<'_> {
    MarkerView {
        id: marker.id,
        position_ms: marker.position,
        name: &marker.name,
    }
}

const fn stinger(stinger: MusicStinger) -> StingerView {
    StingerView {
        trigger_id: stinger.trigger_id,
        segment_id: stinger.segment_id,
    }
}

fn track_detail(track: &MusicTrack) -> TrackDetail<'_> {
    TrackDetail {
        track_kind: match track.kind {
            MusicTrackKind::Normal => "normal",
            MusicTrackKind::Random => "random",
            MusicTrackKind::Sequence => "sequence",
            MusicTrackKind::Switch => "switch",
            MusicTrackKind::Other(_) => "other",
        },
        clips: track.clips.iter().map(clip).collect(),
        switch: track.switch.as_ref().map(track_switch),
    }
}

const fn clip(clip: &MusicClip) -> ClipView {
    ClipView {
        source_id: clip.source_id,
        event_id: clip.event_id,
        play_at_ms: clip.play_at,
        begin_trim_ms: clip.begin_trim,
        end_trim_ms: clip.end_trim,
        source_duration_ms: clip.source_duration,
    }
}

fn track_switch(switch: &MusicTrackSwitch) -> TrackSwitchView<'_> {
    TrackSwitchView {
        group_type: switch.group_type,
        group_kind: switch.group_kind(),
        group_id: switch.group_id,
        default_switch: switch.default_switch,
        switches: &switch.switches,
    }
}

fn switch_detail(switch: &MusicSwitch) -> SwitchDetail<'_> {
    SwitchDetail {
        rule_count: switch.rule_count,
        group_ids: &switch.group_ids,
        targets: &switch.targets,
    }
}

fn playlist_detail(playlist: &MusicPlaylist) -> PlaylistDetail {
    PlaylistDetail {
        rule_count: playlist.rule_count,
        items: playlist
            .items
            .iter()
            .zip(playlist.item_depths())
            .map(|(item, depth)| ItemView {
                depth,
                item_id: item.item_id,
                segment_id: item.segment_id,
                children: item.children,
                play_mode: item.play_mode,
                mode: item.mode(),
                loops: item.loops,
                loop_min: item.loop_min,
                loop_max: item.loop_max,
                weight: item.weight,
                avoid_repeat_count: item.avoid_repeat_count,
                uses_weight: item.uses_weight,
                shuffle: item.shuffle,
            })
            .collect(),
    }
}
