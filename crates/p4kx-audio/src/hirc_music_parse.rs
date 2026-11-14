// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::bank_reader::SectionReader;
use crate::hirc_music::{
    MusicMarker, MusicNode, MusicPlaylist, MusicSegment, MusicStinger, MusicSwitch,
};
use crate::hirc_music_node::{self, BankMusicNodeFields, STINGER_LEN};
use crate::hirc_music_playlist::{self, PLAYLIST_ITEM_LEN};
use crate::hirc_music_segment::{self, BankMusicSegmentFields, finish};
use crate::hirc_music_switch::{
    self, BankMusicSwitchFields, BankMusicSwitchNode, NODE_LEN, SwitchGroups,
};
use crate::hirc_music_switch_traversal::Traversal;
use crate::hirc_music_transition::{self, BankMusicTransitionFields, MusicTransitionFeatures};
use crate::hirc_node::{self, ID_LEN};
use crate::{BankError, SectionId};

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
    let TransitionNode {
        node,
        rule_count,
        transition_features,
    } = transition_node(fields)?;
    let mut observed = BankMusicSwitchFields::default();
    let mut groups = SwitchGroups::default();
    let mut group_ids = Vec::new();
    let tree = hirc_music_switch::header(fields, &mut observed, &mut groups, |id| {
        group_ids.push(id.value);
    })?;
    let size = usize::try_from(tree.byte_len).unwrap_or(usize::MAX);
    let original = fields.bytes(size)?;
    let mut traversal = Traversal::new(tree.checked_nodes()?, tree.depth);
    let mut targets = Vec::new();
    while let Some(target) = traversal.next_target(|index| slice_node(original, index))? {
        targets.push(target.id.value);
    }
    finish(fields)?;
    Ok(MusicSwitch {
        node,
        rule_count,
        transition_features,
        group_ids,
        targets,
    })
}

fn slice_node(tree: &[u8], index: u32) -> Result<BankMusicSwitchNode, BankError> {
    let invalid = || hirc_music_switch::invalid("a decision tree node lies outside the tree");
    let offset = usize::try_from(u64::from(index) * u64::from(NODE_LEN)).map_err(|_| invalid())?;
    let bytes = tree
        .get(offset..)
        .and_then(<[u8]>::first_chunk::<{ NODE_LEN as usize }>)
        .ok_or_else(invalid)?;
    hirc_music_switch::node(&mut SectionReader::new(SectionId::HIRC, bytes))
}

pub(crate) fn playlist(fields: &mut SectionReader<'_>) -> Result<MusicPlaylist, BankError> {
    let TransitionNode {
        node,
        rule_count,
        transition_features,
    } = transition_node(fields)?;
    let count = fields.u32()?;
    let mut items = Vec::with_capacity(capacity(fields, count, PLAYLIST_ITEM_LEN));
    for _ in 0..count {
        items.push(hirc_music_playlist::item(fields)?.value());
    }
    finish(fields)?;
    Ok(MusicPlaylist {
        node,
        rule_count,
        transition_features,
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

struct TransitionNode {
    node: MusicNode,
    rule_count: u32,
    transition_features: MusicTransitionFeatures,
}

fn transition_node(fields: &mut SectionReader<'_>) -> Result<TransitionNode, BankError> {
    let node = node(fields)?;
    let mut observed = BankMusicTransitionFields::default();
    let rule_count = hirc_music_transition::read(fields, &mut observed)?;
    Ok(TransitionNode {
        node,
        rule_count,
        transition_features: observed.features,
    })
}

fn capacity(fields: &SectionReader<'_>, count: u32, item_len: usize) -> usize {
    usize::try_from(count)
        .unwrap_or(usize::MAX)
        .min(fields.remaining() / item_len)
}
