// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::bank_object_fields;
use crate::bank_reader::SectionReader;
use crate::{BankError, HircKind, SectionId};

const ID_LEN: usize = 4;
const FX_LEN: usize = 6;
const METADATA_FX_LEN: usize = 6;
pub(crate) const PROP_LEN: usize = 5;
pub(crate) const RANGED_PROP_LEN: usize = 9;
const VERTEX_LEN: usize = 16;
const PATH_ITEM_LEN: usize = 8;
const AUTOMATION_LEN: usize = 12;
const AUX_BUSES_LEN: usize = 16;
const ADVANCED_SETTINGS_LEN: usize = 6;
const STATE_VALUE_LEN: usize = 6;
const CURVE_POINT_LEN: usize = 12;
const RANDOM_SEQUENCE_PARAMS_LEN: usize = 24;
const PLAYLIST_ITEM_LEN: usize = 8;
const SWITCH_PARAMS_LEN: usize = 13;
const SWITCH_GROUP_LEN: usize = 1 + ID_LEN + ID_LEN + 1;
const PATH_HEADER_LEN: usize = 1 + 4;
const SEQUENCE_MODE: u8 = 1;
const HAS_POSITIONING: u8 = 0b0000_0001;
const HAS_3D: u8 = 0b0000_0010;
const POSITION_TYPE_SHIFT: u8 = 5;
const POSITION_TYPE_MASK: u8 = 0b11;
const HAS_AUX: u8 = 0b0000_1000;

/// The kind of a container object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ContainerKind {
    /// A random container, which plays one of its children at random.
    Random,
    /// A sequence container, which plays its children one after another.
    Sequence,
    /// A switch container, which plays the children of the current switch or state.
    Switch,
    /// A blend container (a layer container in the bank), which plays its children together.
    Blend,
}

/// A random, sequence, switch or blend container object.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct Container {
    /// The kind of container.
    pub kind: ContainerKind,
    /// The ID of the parent node in the actor-mixer hierarchy.
    pub parent_id: u32,
    /// The IDs of the child nodes, in bank order.
    pub children: Vec<u32>,
}

/// Decodes a container body after its ID; every byte of it must be used.
pub(crate) fn container(
    kind: HircKind,
    fields: &mut SectionReader<'_>,
) -> Result<Container, BankError> {
    let parent_id = node_base(fields)?;
    let (kind, children) = match kind {
        HircKind::SwitchContainer => {
            fields.skip(SWITCH_GROUP_LEN)?;
            let children = children(fields)?;
            skip_switch_groups(fields)?;
            let count = fields.u32()?;
            skip_items(fields, count, SWITCH_PARAMS_LEN)?;
            (ContainerKind::Switch, children)
        }
        HircKind::LayerContainer => {
            let children = children(fields)?;
            skip_layers(fields)?;
            fields.u8()?;
            (ContainerKind::Blend, children)
        }
        HircKind::RandomSequenceContainer => {
            fields.skip(RANDOM_SEQUENCE_PARAMS_LEN - 2)?;
            let mode = fields.u8()?;
            fields.u8()?;
            let children = children(fields)?;
            let count = fields.u16()?;
            skip_items(fields, u32::from(count), PLAYLIST_ITEM_LEN)?;
            let kind = if mode == SEQUENCE_MODE {
                ContainerKind::Sequence
            } else {
                ContainerKind::Random
            };
            (kind, children)
        }
        _ => {
            return Err(BankError::invalid(
                SectionId::HIRC,
                "the object is not a container",
            ));
        }
    };
    if fields.remaining() > 0 {
        return Err(BankError::invalid(
            SectionId::HIRC,
            "bytes follow the fields of a container",
        ));
    }
    Ok(Container {
        kind,
        parent_id,
        children,
    })
}

/// Skips the node base parameters of a sound or container and returns its parent ID.
pub(crate) fn node_base(fields: &mut SectionReader<'_>) -> Result<u32, BankError> {
    fields.u8()?;
    let effects = fields.u8()?;
    if effects > 0 {
        fields.u8()?;
        skip_items(fields, u32::from(effects), FX_LEN)?;
    }
    fields.u8()?;
    let metadata = fields.u8()?;
    skip_items(fields, u32::from(metadata), METADATA_FX_LEN)?;
    fields.u32()?;
    let parent_id = fields.u32()?;
    fields.u8()?;
    let props = fields.u8()?;
    skip_items(fields, u32::from(props), PROP_LEN)?;
    let ranged = fields.u8()?;
    skip_items(fields, u32::from(ranged), RANGED_PROP_LEN)?;
    skip_positioning(fields)?;
    if fields.u8()? & HAS_AUX != 0 {
        fields.skip(AUX_BUSES_LEN)?;
    }
    fields.u32()?;
    fields.skip(ADVANCED_SETTINGS_LEN)?;
    skip_states(fields)?;
    skip_rtpcs(fields)?;
    Ok(parent_id)
}

fn skip_positioning(fields: &mut SectionReader<'_>) -> Result<(), BankError> {
    let bits = fields.u8()?;
    if bits & HAS_POSITIONING == 0 || bits & HAS_3D == 0 {
        return Ok(());
    }
    fields.u8()?;
    if (bits >> POSITION_TYPE_SHIFT) & POSITION_TYPE_MASK == 0 {
        return Ok(());
    }
    fields.skip(PATH_HEADER_LEN)?;
    let vertices = fields.u32()?;
    skip_items(fields, vertices, VERTEX_LEN)?;
    let items = fields.u32()?;
    skip_items(fields, items, PATH_ITEM_LEN)?;
    skip_items(fields, items, AUTOMATION_LEN)
}

fn skip_states(fields: &mut SectionReader<'_>) -> Result<(), BankError> {
    for _ in 0..fields.variable_length()? {
        fields.variable_length()?;
        fields.skip(2)?;
    }
    for _ in 0..fields.variable_length()? {
        fields.skip(ID_LEN + 1)?;
        for _ in 0..fields.variable_length()? {
            fields.u32()?;
            let values = fields.u16()?;
            skip_items(fields, u32::from(values), STATE_VALUE_LEN)?;
        }
    }
    Ok(())
}

fn skip_rtpcs(fields: &mut SectionReader<'_>) -> Result<(), BankError> {
    for _ in 0..fields.u16()? {
        fields.skip(ID_LEN + 2)?;
        fields.variable_length()?;
        fields.skip(ID_LEN + 1)?;
        let points = fields.u16()?;
        skip_items(fields, u32::from(points), CURVE_POINT_LEN)?;
    }
    Ok(())
}

fn skip_switch_groups(fields: &mut SectionReader<'_>) -> Result<(), BankError> {
    for _ in 0..fields.u32()? {
        fields.u32()?;
        let nodes = fields.u32()?;
        skip_items(fields, nodes, ID_LEN)?;
    }
    Ok(())
}

fn skip_layers(fields: &mut SectionReader<'_>) -> Result<(), BankError> {
    for _ in 0..fields.u32()? {
        fields.u32()?;
        skip_rtpcs(fields)?;
        fields.skip(ID_LEN + 1)?;
        for _ in 0..fields.u32()? {
            fields.u32()?;
            let points = fields.u32()?;
            skip_items(fields, points, CURVE_POINT_LEN)?;
        }
    }
    Ok(())
}

pub(crate) fn children(fields: &mut SectionReader<'_>) -> Result<Vec<u32>, BankError> {
    let count = usize::try_from(fields.u32()?).unwrap_or(usize::MAX);
    if count > fields.remaining() / ID_LEN {
        return Err(BankError::invalid(
            SectionId::HIRC,
            "a container lists more children than its body holds",
        ));
    }
    let mut children = Vec::with_capacity(count);
    for _ in 0..count {
        children.push(fields.u32()?);
    }
    Ok(children)
}

pub(crate) fn skip_items(
    fields: &mut SectionReader<'_>,
    count: u32,
    len: usize,
) -> Result<(), BankError> {
    fields
        .skip(bank_object_fields::item_bytes(count, len)?)
        .map(|_| ())
}
