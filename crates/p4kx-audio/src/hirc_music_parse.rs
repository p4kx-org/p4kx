// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::bank_reader::SectionReader;
use crate::hirc_music::{
    MusicMarker, MusicNode, MusicPlaylist, MusicSegment, MusicStinger, MusicSwitch,
};
use crate::hirc_music_node::{self, BankMusicNodeFields, STINGER_LEN};
use crate::hirc_music_playlist::{self, PLAYLIST_ITEM_LEN};
use crate::hirc_music_segment::{self, BankMusicSegmentFields, finish};
use crate::hirc_music_transition::{self, BankMusicTransitionFields};
use crate::hirc_node::{self, ID_LEN, skip_items};
use crate::{BankError, SectionId};

const TREE_NODE_LEN: usize = 12;

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
        items.push(hirc_music_playlist::item(fields)?.value());
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
    let rules = hirc_music_transition::read(fields, &mut BankMusicTransitionFields::default())?;
    Ok((node, rules))
}

fn capacity(fields: &SectionReader<'_>, count: u32, item_len: usize) -> usize {
    usize::try_from(count)
        .unwrap_or(usize::MAX)
        .min(fields.remaining() / item_len)
}
