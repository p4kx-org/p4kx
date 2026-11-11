// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use crate::bank_object_fields::{BankHircField, ObjectFields};
use crate::bank_reader::SectionReader;
use crate::hirc_node::{self, BankNodeParentFields, ID_LEN};
use crate::hirc_node_aux::BankNodeAuxFields;
use crate::hirc_node_playback::BankNodePlaybackFields;
use crate::hirc_node_property::BankNodePropertyExtents;
use crate::hirc_rtpc::BankRtpcExtent;
use crate::{BankError, SectionId};

pub(crate) const VERSION: u32 = crate::hirc_source::NODE_BASE_VERSION;

/// The decoded parent and ordered children of a version-154 `ActorMixer`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ActorMixer {
    /// The original parent ID, including zero.
    pub parent_id: u32,
    /// Original child IDs in authored order.
    pub children: Vec<u32>,
    /// Bank-relative extent of the traversed shared node base.
    pub node_range: Range<u64>,
    /// Bank-relative extent of the declared child count.
    pub child_count_range: Range<u64>,
    /// Bank-relative extent of the complete child list.
    pub children_range: Range<u64>,
}

/// Original fields and checked extents of one `ActorMixer` body.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct BankActorMixerFields {
    /// The original fields observed while reading the shared node base.
    pub node: BankNodeParentFields,
    /// Original property and ranged modifier extents when their counts were read.
    pub properties: BankNodePropertyExtents,
    /// Original auxiliary routing fields.
    pub aux: BankNodeAuxFields,
    /// Original positioning and advanced playback settings.
    pub playback: BankNodePlaybackFields,
    /// Traversed node prefix, including checked opaque bytes.
    pub node_range: Option<Range<u64>>,
    /// Complete checked extent of the inline State chunk, when available.
    pub state_chunk: Option<Range<u64>>,
    /// Original count and complete RTPC curve prefix, when available.
    pub rtpc_chunk: Option<BankRtpcExtent>,
    /// Original declared child count, including a malformed count.
    pub child_count: Option<BankHircField<u32>>,
    /// Checked prefix of complete child ID words, including an empty list.
    pub children: Option<Range<u64>>,
    /// Number of complete words in the checked child prefix.
    pub checked_children: u32,
    /// Bytes after a complete child list, including an empty extent.
    pub trailing: Option<Range<u64>>,
}

pub(crate) struct Head {
    parent_id: u32,
    count_start: u64,
    pub(crate) child_bytes: u64,
}

pub(crate) fn head<F: ObjectFields>(
    fields: &mut F,
    observed: &mut BankActorMixerFields,
) -> Result<Head, F::Error> {
    let start = fields.offset();
    let parent = hirc_node::read_base_observed(
        fields,
        &mut observed.node,
        &mut observed.properties,
        &mut observed.aux,
        &mut observed.playback,
        &mut observed.state_chunk,
        &mut observed.rtpc_chunk,
    );
    observed.node_range = Some(start..fields.offset());
    let parent_id = parent?;
    let count = fields.read_u32()?;
    let available = fields.remaining_bytes();
    let authored = count.value;
    let complete_words = u32::try_from(available / ID_LEN as u64).unwrap_or(u32::MAX);
    observed.checked_children = authored.min(complete_words);
    let start = fields.offset();
    observed.children = Some(start..start + u64::from(observed.checked_children) * ID_LEN as u64);
    let count_start = count.range.start;
    observed.child_count = Some(count);
    let child_bytes = hirc_node::checked_child_bytes(authored, available)?;
    Ok(Head {
        parent_id,
        count_start,
        child_bytes,
    })
}

pub(crate) fn finish<F: ObjectFields>(
    fields: &F,
    observed: &mut BankActorMixerFields,
) -> Result<(), F::Error> {
    let start = fields.offset();
    let remaining = fields.remaining_bytes();
    observed.trailing = Some(start..start + remaining);
    if remaining > 0 {
        return Err(BankError::invalid(
            SectionId::HIRC,
            "bytes follow the fields of an ActorMixer",
        )
        .into());
    }
    Ok(())
}

pub(crate) fn read(
    fields: &mut SectionReader<'_>,
    bank_offset: u64,
) -> Result<ActorMixer, BankError> {
    let mut observed = BankActorMixerFields::default();
    let node_start = fields.offset();
    let header = head(fields, &mut observed)?;
    let children_start = fields.offset();
    let children = hirc_node::read_children(fields, observed.checked_children)?;
    let end = fields.offset();
    finish(fields, &mut observed)?;
    let count_start = header.count_start;
    Ok(ActorMixer {
        parent_id: header.parent_id,
        children,
        node_range: bank_offset + node_start..bank_offset + count_start,
        child_count_range: bank_offset + count_start..bank_offset + children_start,
        children_range: bank_offset + children_start..bank_offset + end,
    })
}
