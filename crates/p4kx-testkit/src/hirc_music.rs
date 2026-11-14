// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::TestHircObject;
use crate::TestNodeBase;
use crate::bytes::{put_u16, put_u32};
use crate::hirc_node::node_base;

const VORBIS_PLUGIN_ID: u32 = 0x0004_0001;
const DEFAULT_STINGER: (u32, u32) = (0x5100_0001, 0x5100_0002);
const SWITCH_GROUP: u8 = 0;
const STATE_GROUP: u8 = 1;
const MARKER_SPACING_MS: f64 = 250.0;
const FIRST_MARKER_ID: u32 = 0x100;

impl TestHircObject {
    /// A music segment object laid out as bank version 154 writes it: `children` are its tracks,
    /// `markers` the names of its markers.
    #[must_use]
    pub fn music_segment(id: u32, children: &[u32], markers: &[&str]) -> Self {
        Self::segment(id, children, markers, &[DEFAULT_STINGER])
    }

    /// A music segment object without markers whose one stinger plays the segment `segment_id`
    /// when the trigger `trigger_id` fires.
    #[must_use]
    pub fn music_segment_stinging(
        id: u32,
        children: &[u32],
        trigger_id: u32,
        segment_id: u32,
    ) -> Self {
        Self::segment(id, children, &[], &[(trigger_id, segment_id)])
    }

    /// A music segment whose authored stingers are the ordered trigger/segment ID pairs.
    #[must_use]
    pub fn music_segment_with_stingers(id: u32, children: &[u32], stingers: &[(u32, u32)]) -> Self {
        Self::segment(id, children, &[], stingers)
    }

    fn segment(id: u32, children: &[u32], markers: &[&str], stingers: &[(u32, u32)]) -> Self {
        let mut body = music_node(children, stingers);
        body.extend_from_slice(&4000.5_f64.to_le_bytes());
        put_u32(&mut body, len_u32(markers));
        for (id, name) in (FIRST_MARKER_ID..).zip(markers) {
            put_u32(&mut body, id);
            let position = MARKER_SPACING_MS.mul_add(f64::from(id - FIRST_MARKER_ID), 1.5);
            body.extend_from_slice(&position.to_le_bytes());
            body.extend_from_slice(name.as_bytes());
            body.push(0);
        }
        Self::raw(10, id, body)
    }

    /// A music track object playing the media `sources`, streamed, laid out as bank version 154
    /// writes it.
    #[must_use]
    pub fn music_track(id: u32, sources: &[u32]) -> Self {
        Self::raw(11, id, track(sources, None))
    }

    /// A music switch track: like [`music_track`](Self::music_track), following the switch group
    /// `group_id` and picking a clip by one of `switches`.
    #[must_use]
    pub fn music_switch_track(id: u32, sources: &[u32], group_id: u32, switches: &[u32]) -> Self {
        Self::raw(
            11,
            id,
            track(sources, Some((SWITCH_GROUP, group_id, switches))),
        )
    }

    /// A music switch track following the state group `group_id` instead of a switch group.
    #[must_use]
    pub fn music_state_track(id: u32, sources: &[u32], group_id: u32, states: &[u32]) -> Self {
        Self::raw(
            11,
            id,
            track(sources, Some((STATE_GROUP, group_id, states))),
        )
    }

    /// A music switch container whose decision tree has one level: its leaves play `targets`.
    /// It lists `children` and two transition rules, the second with a transition object.
    #[must_use]
    pub fn music_switch(id: u32, children: &[u32], group_id: u32, targets: &[u32]) -> Self {
        let mut tree = Vec::new();
        put_u32(&mut tree, 0);
        put_u16(&mut tree, 1);
        put_u16(&mut tree, u16::try_from(targets.len()).unwrap_or(u16::MAX));
        put_u16(&mut tree, 50);
        put_u16(&mut tree, 100);
        for (key, target) in (0x7000_u32..).zip(targets) {
            put_u32(&mut tree, key);
            put_u32(&mut tree, *target);
            put_u16(&mut tree, 50);
            put_u16(&mut tree, 100);
        }
        Self::music_switch_with_tree(id, children, &[(group_id, SWITCH_GROUP)], &tree)
    }

    /// A music switch with original ordered group IDs, group types and decision tree bytes.
    #[must_use]
    pub fn music_switch_with_tree(
        id: u32,
        children: &[u32],
        groups: &[(u32, u8)],
        tree: &[u8],
    ) -> Self {
        let mut body = transition_node(children);
        body.push(1);
        put_u32(&mut body, len_u32(groups));
        for &(id, _) in groups {
            put_u32(&mut body, id);
        }
        body.extend(groups.iter().map(|&(_, code)| code));
        put_u32(&mut body, len_u32(tree));
        body.push(0);
        body.extend_from_slice(tree);
        Self::raw(12, id, body)
    }

    /// A music playlist container with one item per child segment, two transition rules, the
    /// second with a transition object.
    #[must_use]
    pub fn music_playlist(id: u32, children: &[u32]) -> Self {
        let items: Vec<(u32, u32)> = children.iter().map(|child| (*child, 0)).collect();
        Self::music_playlist_tree(id, children, &items)
    }

    /// A music playlist container like [`music_playlist`](Self::music_playlist) whose `items` are
    /// given as pairs of segment ID (0 for a group) and number of child items that follow.
    #[must_use]
    pub fn music_playlist_tree(id: u32, children: &[u32], items: &[(u32, u32)]) -> Self {
        let items: Vec<TestPlaylistItem> = items
            .iter()
            .map(|(segment, child_items)| TestPlaylistItem::new(*segment, *child_items))
            .collect();
        Self::music_playlist_items(id, children, &items)
    }

    /// A music playlist container like [`music_playlist`](Self::music_playlist) with the given
    /// `items`.
    #[must_use]
    pub fn music_playlist_items(id: u32, children: &[u32], items: &[TestPlaylistItem]) -> Self {
        let mut body = transition_node(children);
        put_u32(&mut body, len_u32(items));
        for (item, entry) in (1_u32..).zip(items) {
            put_u32(&mut body, entry.segment);
            put_u32(&mut body, item);
            put_u32(&mut body, entry.child_items);
            put_u32(&mut body, entry.play_mode);
            body.extend_from_slice(&entry.loops.to_le_bytes());
            body.extend_from_slice(&2_i16.to_le_bytes());
            body.extend_from_slice(&5_i16.to_le_bytes());
            put_u32(&mut body, 50_000);
            put_u16(&mut body, 1);
            body.extend_from_slice(&[1, 0]);
        }
        Self::raw(13, id, body)
    }
}

/// One item of a synthetic music playlist: a segment (0 for a group), the number of child items
/// that follow it, its play mode code (default 1) and its loop count (default -1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TestPlaylistItem {
    segment: u32,
    child_items: u32,
    play_mode: u32,
    loops: i16,
}

impl TestPlaylistItem {
    /// An item of `segment` followed by `child_items` child items.
    #[must_use]
    pub const fn new(segment: u32, child_items: u32) -> Self {
        Self {
            segment,
            child_items,
            play_mode: 1,
            loops: -1,
        }
    }

    /// Sets the play mode code.
    #[must_use]
    pub const fn play_mode(mut self, play_mode: u32) -> Self {
        self.play_mode = play_mode;
        self
    }

    /// Sets the loop count, 0 for endlessly.
    #[must_use]
    pub const fn loops(mut self, loops: i16) -> Self {
        self.loops = loops;
        self
    }
}

fn music_node(children: &[u32], stingers: &[(u32, u32)]) -> Vec<u8> {
    let mut body = vec![0];
    body.extend_from_slice(&node_base(TestNodeBase::Full));
    put_u32(&mut body, len_u32(children));
    for child in children {
        put_u32(&mut body, *child);
    }
    body.extend_from_slice(&1000.0_f64.to_le_bytes());
    body.extend_from_slice(&0.0_f64.to_le_bytes());
    body.extend_from_slice(&120.0_f32.to_le_bytes());
    body.extend_from_slice(&[4, 4]);
    body.push(1);
    put_u32(&mut body, len_u32(stingers));
    for &(trigger_id, segment_id) in stingers {
        put_u32(&mut body, trigger_id);
        put_u32(&mut body, segment_id);
        body.extend_from_slice(&[0; 16]);
    }
    body
}

fn transition_node(children: &[u32]) -> Vec<u8> {
    let mut body = music_node(children, &[DEFAULT_STINGER]);
    put_u32(&mut body, 2);
    for with_object in [false, true] {
        put_u32(&mut body, 1);
        put_u32(&mut body, 0x5200_0001);
        put_u32(&mut body, 2);
        put_u32(&mut body, 0x5200_0002);
        put_u32(&mut body, 0x5200_0003);
        body.extend_from_slice(&[0; 21 + 26]);
        body.push(u8::from(with_object));
        if with_object {
            body.extend_from_slice(&[0; 30]);
        }
    }
    body
}

fn track(sources: &[u32], switch: Option<(u8, u32, &[u32])>) -> Vec<u8> {
    let mut body = Vec::new();
    put_u32(&mut body, len_u32(sources));
    for source in sources {
        put_u32(&mut body, VORBIS_PLUGIN_ID);
        body.push(2);
        put_u32(&mut body, *source);
        put_u32(&mut body, source ^ 0x5A5A_5A5A);
        put_u32(&mut body, 4096);
        body.push(0);
    }
    body.push(0);
    put_u32(&mut body, len_u32(sources));
    for (clip, source) in (1_u32..).zip(sources) {
        put_u32(&mut body, clip);
        put_u32(&mut body, *source);
        put_u32(&mut body, source ^ 0x5A5A_5A5A);
        put_u32(&mut body, 0);
        for seconds in [0.0_f64, 0.5, 0.25, 12.0] {
            body.extend_from_slice(&seconds.to_le_bytes());
        }
    }
    if !sources.is_empty() {
        put_u32(&mut body, 0);
    }
    put_u32(&mut body, 1);
    put_u32(&mut body, 0);
    put_u32(&mut body, 0);
    put_u32(&mut body, 2);
    for _ in 0..2 {
        body.extend_from_slice(&[0; 12]);
    }
    body.extend_from_slice(&node_base(TestNodeBase::Plain));
    if let Some((group_type, group_id, switches)) = switch {
        body.push(3);
        body.push(group_type);
        put_u32(&mut body, group_id);
        put_u32(&mut body, 0);
        put_u32(&mut body, len_u32(switches));
        for switch in switches {
            put_u32(&mut body, *switch);
        }
        body.extend_from_slice(&[0; 32]);
    } else {
        body.push(0);
    }
    body.extend_from_slice(&0_i32.to_le_bytes());
    body
}

fn len_u32<T>(items: &[T]) -> u32 {
    u32::try_from(items.len()).unwrap_or(u32::MAX)
}
