// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use crate::bank_object_fields::{BankHircField, ObjectFields};
use crate::hirc_music_node::{BankMusicNodeFields, MusicList};
use crate::hirc_music_transition::BankMusicTransitionFields;
use crate::{BankError, SectionId};

pub(crate) const NODE_LEN: u32 = 12;

/// Original fields locating a music switch decision tree.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct BankMusicSwitchFields {
    /// Original music node fields.
    pub node: BankMusicNodeFields,
    /// Original transition rule count and opaque extent.
    pub transitions: BankMusicTransitionFields,
    /// Original continue-playback control byte.
    pub continue_playback_code: Option<BankHircField<u8>>,
    /// Original decision tree depth and group count.
    pub group_count: Option<BankHircField<u32>>,
    /// Original decision tree byte length.
    pub tree_size: Option<BankHircField<u32>>,
    /// Original decision mode code.
    pub tree_mode_code: Option<BankHircField<u8>>,
    /// Admitted original decision tree extent.
    pub tree_range: Option<Range<u64>>,
}

/// Original group ID and its independently checked type byte.
#[derive(Debug)]
#[non_exhaustive]
pub struct BankMusicSwitchGroup {
    /// Original group ID.
    pub id: BankHircField<u32>,
    /// Original type byte when that part of the source is readable.
    pub type_code: Option<BankHircField<u8>>,
}

/// Original words of one physical decision node.
#[derive(Debug)]
#[non_exhaustive]
pub struct BankMusicSwitchNode {
    /// Original decision key.
    pub key: BankHircField<u32>,
    /// Original target ID or packed child index/count word.
    pub link: BankHircField<u32>,
    /// Original weight.
    pub weight: BankHircField<u16>,
    /// Original probability.
    pub probability: BankHircField<u16>,
    /// Original complete node extent.
    pub range: Range<u64>,
}

#[derive(Default)]
pub(crate) struct SwitchGroups {
    pub(crate) ids: MusicList,
    pub(crate) types: MusicList,
}

#[derive(Clone, Copy)]
pub(crate) struct DecisionTree {
    pub(crate) start: u64,
    pub(crate) byte_len: u32,
    pub(crate) depth: u32,
}

impl DecisionTree {
    pub(crate) fn range(self) -> Range<u64> {
        self.start..self.start + u64::from(self.byte_len)
    }

    pub(crate) const fn physical_nodes(self) -> u32 {
        self.byte_len / NODE_LEN
    }

    pub(crate) const fn checked_nodes(self) -> Result<u32, BankError> {
        if self.byte_len.is_multiple_of(NODE_LEN) {
            Ok(self.physical_nodes())
        } else {
            Err(invalid("a decision tree is no whole number of nodes"))
        }
    }
}

pub(crate) fn header<F: ObjectFields>(
    input: &mut F,
    observed: &mut BankMusicSwitchFields,
    groups: &mut SwitchGroups,
    mut emit: impl FnMut(BankHircField<u32>),
) -> Result<DecisionTree, F::Error> {
    observed.continue_playback_code = Some(input.read_u8()?);
    let count = input.read_u32()?;
    let depth = count.value;
    observed.group_count = Some(count);
    groups.ids.start = input.offset();
    for _ in 0..depth {
        let id = input.read_u32()?;
        groups.ids.record(input.offset());
        emit(id);
    }
    let available = u32::try_from(input.remaining_bytes())
        .unwrap_or(u32::MAX)
        .min(depth);
    groups.types = MusicList {
        start: input.offset(),
        length: u64::from(available),
        count: available,
    };
    input.skip_bytes(u64::from(depth))?;
    let size = input.read_u32()?;
    let byte_len = size.value;
    observed.tree_size = Some(size);
    observed.tree_mode_code = Some(input.read_u8()?);
    Ok(DecisionTree {
        start: input.offset(),
        byte_len,
        depth,
    })
}

pub(crate) fn node<F: ObjectFields>(input: &mut F) -> Result<BankMusicSwitchNode, F::Error> {
    let start = input.offset();
    Ok(BankMusicSwitchNode {
        key: input.read_u32()?,
        link: input.read_u32()?,
        weight: input.read_u16()?,
        probability: input.read_u16()?,
        range: start..input.offset(),
    })
}

pub(crate) const fn invalid(reason: &'static str) -> BankError {
    BankError::invalid(SectionId::HIRC, reason)
}
