// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::bank_reader::SectionReader;
use crate::hirc::{self, MediaStorage, SoundSource};
use crate::hirc_music::{
    MusicClip, MusicMarker, MusicNode, MusicPlaylist, MusicPlaylistItem, MusicSegment,
    MusicStinger, MusicSwitch, MusicTrack, MusicTrackKind, MusicTrackSwitch,
};
use crate::hirc_music_node::{self, BankMusicNodeFields, STINGER_LEN};
use crate::hirc_music_segment::{self, BankMusicSegmentFields, finish};
use crate::hirc_node::{self, ID_LEN, children, skip_items};
use crate::{BankError, SectionId};

const CLIP_LEN: usize = 16 + 32;
const CLIP_WITHOUT_CACHE_ID_LEN: usize = CLIP_LEN - ID_LEN;
const SOURCE_LEN: usize = 4 + 1 + 4 + 4 + 4 + 1;
const AUTOMATION_POINT_LEN: usize = 12;
const SWITCH_TRANSITION_LEN: usize = 32;
const RULE_SOURCE_LEN: usize = 21;
const RULE_DESTINATION_LEN: usize = 26;
const RULE_OBJECT_LEN: usize = 30;
const MIN_RULE_LEN: usize = 2 * ID_LEN + RULE_SOURCE_LEN + RULE_DESTINATION_LEN + 1;
const TREE_NODE_LEN: usize = 12;
const PLAYLIST_ITEM_LEN: usize = 30;
const SOURCE_PLUGIN_TYPE: u32 = 2;
const PLUGIN_TYPE_MASK: u32 = 0x0F;
const SWITCH_TRACK: u8 = 3;

pub(crate) fn segment(fields: &mut SectionReader<'_>) -> Result<MusicSegment, BankError> {
    let node = node(fields)?;
    let (duration, count) =
        hirc_music_segment::header(fields, &mut BankMusicSegmentFields::default())?;
    let mut markers = Vec::with_capacity(capacity(fields, count, ID_LEN + 8 + 1));
    for _ in 0..count {
        let words = hirc_music_segment::marker(fields)?;
        let name = String::from_utf8_lossy(fields.c_string()?).into();
        markers.push(MusicMarker {
            id: words.id.value,
            position: f64::from_bits(words.position.value),
            name,
        });
    }
    finish(fields)?;
    Ok(MusicSegment {
        node,
        duration,
        markers,
    })
}

fn sources(version: u32, fields: &mut SectionReader<'_>) -> Result<Vec<SoundSource>, BankError> {
    let count = fields.u32()?;
    let mut sources = Vec::with_capacity(capacity(fields, count, SOURCE_LEN));
    for _ in 0..count {
        let plugin_id = fields.u32()?;
        let storage = MediaStorage::from_byte(fields.u8()?);
        let source_id = fields.u32()?;
        if hirc::has_cache_id(version) {
            fields.u32()?;
        }
        let in_memory_size = fields.u32()?;
        let source_bits = fields.u8()?;
        if plugin_id & PLUGIN_TYPE_MASK == SOURCE_PLUGIN_TYPE {
            let len = usize::try_from(fields.u32()?).unwrap_or(usize::MAX);
            fields.skip(len)?;
        }
        sources.push(SoundSource {
            plugin_id,
            storage,
            source_id,
            in_memory_size,
            source_bits,
            parent_id: None,
        });
    }
    Ok(sources)
}

fn track_switch(fields: &mut SectionReader<'_>) -> Result<MusicTrackSwitch, BankError> {
    let group_type = fields.u8()?;
    let group_id = fields.u32()?;
    let default_switch = fields.u32()?;
    let switches = children(fields)?;
    fields.skip(SWITCH_TRANSITION_LEN)?;
    Ok(MusicTrackSwitch {
        group_type,
        group_id,
        default_switch,
        switches,
    })
}

pub(crate) fn track(version: u32, fields: &mut SectionReader<'_>) -> Result<MusicTrack, BankError> {
    let sources = sources(version, fields)?;
    fields.u8()?;
    let clips = clips(version, fields)?;
    let automations = fields.u32()?;
    for _ in 0..automations {
        fields.skip(2 * ID_LEN)?;
        let points = fields.u32()?;
        skip_items(fields, points, AUTOMATION_POINT_LEN)?;
    }
    let parent_id = hirc_node::node_base(fields)?;
    let kind = match fields.u8()? {
        0 => MusicTrackKind::Normal,
        1 => MusicTrackKind::Random,
        2 => MusicTrackKind::Sequence,
        SWITCH_TRACK => MusicTrackKind::Switch,
        other => MusicTrackKind::Other(other),
    };
    let switch = if kind == MusicTrackKind::Switch {
        Some(track_switch(fields)?)
    } else {
        None
    };
    fields.i32()?;
    finish(fields)?;
    Ok(MusicTrack {
        parent_id,
        sources,
        clips,
        kind,
        switch,
    })
}

fn clips(version: u32, fields: &mut SectionReader<'_>) -> Result<Vec<MusicClip>, BankError> {
    let count = fields.u32()?;
    let cache_id = hirc::has_cache_id(version);
    let len = if cache_id {
        CLIP_LEN
    } else {
        CLIP_WITHOUT_CACHE_ID_LEN
    };
    let mut clips = Vec::with_capacity(capacity(fields, count, len));
    for _ in 0..count {
        fields.u32()?;
        let source_id = fields.u32()?;
        if cache_id {
            fields.u32()?;
        }
        let event_id = fields.u32()?;
        clips.push(MusicClip {
            source_id,
            event_id,
            play_at: fields.f64()?,
            begin_trim: fields.f64()?,
            end_trim: fields.f64()?,
            source_duration: fields.f64()?,
        });
    }
    if count > 0 {
        fields.u32()?;
    }
    Ok(clips)
}

pub(crate) fn switch(fields: &mut SectionReader<'_>) -> Result<MusicSwitch, BankError> {
    let (node, rule_count) = transition_node(fields)?;
    fields.u8()?;
    let depth = fields.u32()?;
    let mut group_ids = Vec::with_capacity(capacity(fields, depth, ID_LEN + 1));
    for _ in 0..depth {
        group_ids.push(fields.u32()?);
    }
    skip_items(fields, depth, 1)?;
    let size = usize::try_from(fields.u32()?).unwrap_or(usize::MAX);
    fields.u8()?;
    let tree = fields.bytes(size)?;
    let targets = targets(tree, depth)?;
    finish(fields)?;
    Ok(MusicSwitch {
        node,
        rule_count,
        group_ids,
        targets,
    })
}

fn targets(tree: &[u8], depth: u32) -> Result<Vec<u32>, BankError> {
    let invalid = |reason| BankError::invalid(SectionId::HIRC, reason);
    if !tree.len().is_multiple_of(TREE_NODE_LEN) {
        return Err(invalid("a decision tree is no whole number of nodes"));
    }
    let nodes = tree.len() / TREE_NODE_LEN;
    let mut targets = Vec::new();
    if nodes == 0 {
        return Ok(targets);
    }
    let mut pending = vec![(0_usize, 0_u32)];
    let mut visited = 0_usize;
    while let Some((index, level)) = pending.pop() {
        visited += 1;
        let node = tree
            .get(index * TREE_NODE_LEN..)
            .and_then(<[u8]>::first_chunk::<TREE_NODE_LEN>)
            .ok_or_else(|| invalid("a decision tree node lies outside the tree"))?;
        let [_, _, _, _, a, b, c, d, ..] = *node;
        if level >= depth {
            let target = u32::from_le_bytes([a, b, c, d]);
            if target != 0 {
                targets.push(target);
            }
            continue;
        }
        let first = usize::from(u16::from_le_bytes([a, b]));
        let count = usize::from(u16::from_le_bytes([c, d]));
        if first.checked_add(count).is_none_or(|end| end > nodes) {
            return Err(invalid(
                "a decision tree node lists children outside the tree",
            ));
        }
        if pending.len().saturating_add(count) > nodes - visited {
            return Err(invalid(
                "a decision tree lists more children than it has nodes left",
            ));
        }
        pending.extend((first..first + count).rev().map(|child| (child, level + 1)));
    }
    Ok(targets)
}

pub(crate) fn playlist(fields: &mut SectionReader<'_>) -> Result<MusicPlaylist, BankError> {
    let (node, rule_count) = transition_node(fields)?;
    let count = fields.u32()?;
    let mut items = Vec::with_capacity(capacity(fields, count, PLAYLIST_ITEM_LEN));
    for _ in 0..count {
        items.push(MusicPlaylistItem {
            segment_id: fields.u32()?,
            item_id: fields.u32()?,
            children: fields.u32()?,
            play_mode: fields.u32()?,
            loops: fields.i16()?,
            loop_min: fields.i16()?,
            loop_max: fields.i16()?,
            weight: fields.u32()?,
            avoid_repeat_count: fields.u16()?,
            uses_weight: fields.u8()? != 0,
            shuffle: fields.u8()? != 0,
        });
    }
    finish(fields)?;
    Ok(MusicPlaylist {
        node,
        rule_count,
        items,
    })
}

fn node(fields: &mut SectionReader<'_>) -> Result<MusicNode, BankError> {
    let mut original = BankMusicNodeFields::default();
    let (parent_id, count, _) = hirc_music_node::head(fields, &mut original)?;
    let children = hirc_node::read_children(fields, count)?;
    let meter = hirc_music_node::meter(fields, &mut original)?;
    let count = hirc_music_node::stinger_count(fields, &mut original)?;
    let mut stingers = Vec::with_capacity(capacity(fields, count, STINGER_LEN));
    for _ in 0..count {
        let original = hirc_music_node::stinger(fields)?;
        stingers.push(MusicStinger {
            trigger_id: original.trigger_id.value,
            segment_id: original.segment_id.value,
        });
    }
    Ok(MusicNode {
        parent_id,
        children,
        meter,
        stingers,
    })
}

fn transition_node(fields: &mut SectionReader<'_>) -> Result<(MusicNode, u32), BankError> {
    let node = node(fields)?;
    let rules = fields.u32()?;
    if usize::try_from(rules)
        .ok()
        .is_none_or(|rules| rules > fields.remaining() / MIN_RULE_LEN)
    {
        return Err(BankError::invalid(
            SectionId::HIRC,
            "a music node lists more transition rules than its body holds",
        ));
    }
    for _ in 0..rules {
        for _ in 0..2 {
            let ids = fields.u32()?;
            skip_items(fields, ids, ID_LEN)?;
        }
        fields.skip(RULE_SOURCE_LEN + RULE_DESTINATION_LEN)?;
        if fields.u8()? != 0 {
            fields.skip(RULE_OBJECT_LEN)?;
        }
    }
    Ok((node, rules))
}

fn capacity(fields: &SectionReader<'_>, count: u32, item_len: usize) -> usize {
    usize::try_from(count)
        .unwrap_or(usize::MAX)
        .min(fields.remaining() / item_len)
}
