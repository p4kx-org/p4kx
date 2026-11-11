// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use crate::bank_object_fields::ObjectFields;
use crate::bank_reader::SectionReader;
use crate::hirc_node::{
    self, BankNodeParentFields, CURVE_POINT_LEN, Container, ContainerKind, ID_LEN,
};
use crate::hirc_node_parameters::BankNodeParameters;
use crate::{BankError, BankHircField, HircKind, SectionId};

const RANDOM_SEQUENCE_BEFORE_MODE: u64 = 22;
const SEQUENCE_MODE: u8 = 1;
const SWITCH_GROUP_LEN: u64 = 10;
const SWITCH_PARAMETERS_LEN: usize = 13;
const PLAYLIST_ITEM_LEN: usize = 8;
const LAYER_ASSOCIATION_LEN: u64 = 5;

/// Original fields and checked extents of one container body.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct BankContainerFields {
    /// Original fields read by the shared node parser.
    pub node: BankNodeParentFields,
    /// Checked shared node fields before the container-specific continuation.
    pub parameters: BankNodeParameters,
    /// Traversed node prefix, including checked opaque bytes.
    pub node_range: Option<Range<u64>>,
    /// Raw Random/Sequence mode when present.
    pub mode: Option<BankHircField<u8>>,
    /// The decoded container kind when selected.
    pub kind: Option<ContainerKind>,
    /// Original child count when readable.
    pub child_count: Option<BankHircField<u32>>,
    /// Admitted child ID range, including an empty range.
    pub children: Option<Range<u64>>,
    /// Original bytes after the admitted child IDs.
    pub suffix: Option<Range<u64>>,
}

#[derive(Clone, Copy)]
pub(crate) enum Layout {
    RandomSequence,
    Switch,
    Blend,
}

impl Layout {
    pub(crate) const fn of(kind: HircKind) -> Option<Self> {
        match kind {
            HircKind::RandomSequenceContainer => Some(Self::RandomSequence),
            HircKind::SwitchContainer => Some(Self::Switch),
            HircKind::LayerContainer => Some(Self::Blend),
            _ => None,
        }
    }
}

pub(crate) fn container(
    kind: HircKind,
    fields: &mut SectionReader<'_>,
) -> Result<Container, BankError> {
    let layout = Layout::of(kind)
        .ok_or_else(|| BankError::invalid(SectionId::HIRC, "the object is not a container"))?;
    let mut original = BankContainerFields::default();
    let header = head(layout, fields, &mut original)?;
    let children = hirc_node::read_children(fields, header.count)?;
    tail(layout, fields, &mut original)?;
    Ok(Container {
        kind: header.kind,
        parent_id: header.parent_id,
        children,
    })
}

pub(crate) struct Head {
    pub(crate) kind: ContainerKind,
    pub(crate) parent_id: u32,
    pub(crate) count: u32,
    pub(crate) child_bytes: u64,
}

pub(crate) fn head<F: ObjectFields>(
    layout: Layout,
    fields: &mut F,
    observed: &mut BankContainerFields,
) -> Result<Head, F::Error> {
    let start = fields.offset();
    let parent =
        hirc_node::read_base_parameters(fields, &mut observed.node, &mut observed.parameters);
    observed.node_range = Some(start..fields.offset());
    let parent_id = parent?;
    let kind = container_kind(layout, fields, observed)?;
    let count = fields.read_u32()?;
    let authored = count.value;
    observed.child_count = Some(count);
    let child_bytes = hirc_node::checked_child_bytes(authored, fields.remaining_bytes())?;
    let start = fields.offset();
    observed.children = Some(start..start + child_bytes);
    Ok(Head {
        kind,
        parent_id,
        count: authored,
        child_bytes,
    })
}

fn container_kind<F: ObjectFields>(
    layout: Layout,
    fields: &mut F,
    observed: &mut BankContainerFields,
) -> Result<ContainerKind, F::Error> {
    let kind = match layout {
        Layout::Switch => {
            fields.skip_bytes(SWITCH_GROUP_LEN)?;
            ContainerKind::Switch
        }
        Layout::Blend => ContainerKind::Blend,
        Layout::RandomSequence => {
            fields.skip_bytes(RANDOM_SEQUENCE_BEFORE_MODE)?;
            let mode = fields.read_u8()?;
            let kind = if mode.value == SEQUENCE_MODE {
                ContainerKind::Sequence
            } else {
                ContainerKind::Random
            };
            observed.mode = Some(mode);
            observed.kind = Some(kind);
            fields.read_array::<1>()?;
            kind
        }
    };
    observed.kind = Some(kind);
    Ok(kind)
}

pub(crate) fn tail<F: ObjectFields>(
    layout: Layout,
    fields: &mut F,
    observed: &mut BankContainerFields,
) -> Result<(), F::Error> {
    let start = fields.offset();
    observed.suffix = Some(start..start + fields.remaining_bytes());
    match layout {
        Layout::Switch => switch_tail(fields)?,
        Layout::Blend => blend_tail(fields)?,
        Layout::RandomSequence => {
            let items = fields.read_u16()?.value;
            hirc_node::skip_items(fields, u32::from(items), PLAYLIST_ITEM_LEN)?;
        }
    }
    if fields.remaining_bytes() > 0 {
        return Err(
            BankError::invalid(SectionId::HIRC, "bytes follow the fields of a container").into(),
        );
    }
    Ok(())
}

fn switch_tail<F: ObjectFields>(fields: &mut F) -> Result<(), F::Error> {
    for _ in 0..fields.read_u32()?.value {
        fields.read_array::<4>()?;
        let count = fields.read_u32()?.value;
        hirc_node::skip_items(fields, count, ID_LEN)?;
    }
    let count = fields.read_u32()?.value;
    hirc_node::skip_items(fields, count, SWITCH_PARAMETERS_LEN)
}

fn blend_tail<F: ObjectFields>(fields: &mut F) -> Result<(), F::Error> {
    for _ in 0..fields.read_u32()?.value {
        fields.read_array::<4>()?;
        hirc_node::skip_rtpcs(fields)?;
        fields.skip_bytes(LAYER_ASSOCIATION_LEN)?;
        for _ in 0..fields.read_u32()?.value {
            fields.read_array::<4>()?;
            let count = fields.read_u32()?.value;
            hirc_node::skip_items(fields, count, CURVE_POINT_LEN)?;
        }
    }
    fields.read_array::<1>()?;
    Ok(())
}
