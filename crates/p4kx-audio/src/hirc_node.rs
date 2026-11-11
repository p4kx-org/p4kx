// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::bank_object_fields::{self, BankHircField, ObjectFields};
use crate::bank_reader::SectionReader;
use crate::{BankError, SectionId};

pub(crate) const ID_LEN: usize = 4;
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
pub(crate) const CURVE_POINT_LEN: usize = 12;
const PATH_HEADER_LEN: usize = 1 + 4;
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

/// Original words that locate and supply the parent in a node base.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct BankNodeParentFields {
    /// Original effect count when readable.
    pub effect_count: Option<BankHircField<u8>>,
    /// Original metadata effect count when readable.
    pub metadata_count: Option<BankHircField<u8>>,
    /// Original word immediately before the parent ID.
    pub pre_parent_word: Option<BankHircField<u32>>,
    /// Original parent ID, including zero, when readable.
    pub parent_id: Option<BankHircField<u32>>,
}

pub(crate) fn read_base<F: ObjectFields>(
    fields: &mut F,
    observed: &mut BankNodeParentFields,
) -> Result<u32, F::Error> {
    let parent = read_parent(fields, observed)?;
    fields.read_array::<1>()?;
    let props = fields.read_u8()?.value;
    skip_items(fields, u32::from(props), PROP_LEN)?;
    let ranged = fields.read_u8()?.value;
    skip_items(fields, u32::from(ranged), RANGED_PROP_LEN)?;
    skip_positioning(fields)?;
    if fields.read_u8()?.value & HAS_AUX != 0 {
        fields.skip_bytes(AUX_BUSES_LEN as u64)?;
    }
    fields.read_array::<4>()?;
    fields.skip_bytes(ADVANCED_SETTINGS_LEN as u64)?;
    skip_states(fields)?;
    skip_rtpcs(fields)?;
    Ok(parent)
}

fn read_parent<F: ObjectFields>(
    fields: &mut F,
    observed: &mut BankNodeParentFields,
) -> Result<u32, F::Error> {
    fields.read_array::<1>()?;
    let effects = fields.read_u8()?;
    let count = u32::from(effects.value);
    observed.effect_count = Some(effects);
    if count > 0 {
        fields.read_array::<1>()?;
        skip_items(fields, count, FX_LEN)?;
    }
    fields.read_array::<1>()?;
    let metadata = fields.read_u8()?;
    let count = u32::from(metadata.value);
    observed.metadata_count = Some(metadata);
    skip_items(fields, count, METADATA_FX_LEN)?;
    observed.pre_parent_word = Some(fields.read_u32()?);
    let parent = fields.read_u32()?;
    let id = parent.value;
    observed.parent_id = Some(parent);
    Ok(id)
}

fn skip_positioning<F: ObjectFields>(fields: &mut F) -> Result<(), F::Error> {
    let bits = fields.read_u8()?.value;
    if bits & HAS_POSITIONING == 0 || bits & HAS_3D == 0 {
        return Ok(());
    }
    fields.read_array::<1>()?;
    if (bits >> POSITION_TYPE_SHIFT) & POSITION_TYPE_MASK == 0 {
        return Ok(());
    }
    fields.skip_bytes(PATH_HEADER_LEN as u64)?;
    let vertices = fields.read_u32()?.value;
    skip_items(fields, vertices, VERTEX_LEN)?;
    let items = fields.read_u32()?.value;
    skip_items(fields, items, PATH_ITEM_LEN)?;
    skip_items(fields, items, AUTOMATION_LEN)
}

fn skip_states<F: ObjectFields>(fields: &mut F) -> Result<(), F::Error> {
    for _ in 0..fields.read_variable_length(SectionId::HIRC)?.value {
        fields.read_variable_length(SectionId::HIRC)?;
        fields.skip_bytes(2)?;
    }
    for _ in 0..fields.read_variable_length(SectionId::HIRC)?.value {
        fields.skip_bytes((ID_LEN + 1) as u64)?;
        for _ in 0..fields.read_variable_length(SectionId::HIRC)?.value {
            fields.read_array::<4>()?;
            let values = fields.read_u16()?.value;
            skip_items(fields, u32::from(values), STATE_VALUE_LEN)?;
        }
    }
    Ok(())
}

pub(crate) fn skip_rtpcs<F: ObjectFields>(fields: &mut F) -> Result<(), F::Error> {
    for _ in 0..fields.read_u16()?.value {
        fields.skip_bytes((ID_LEN + 2) as u64)?;
        fields.read_variable_length(SectionId::HIRC)?;
        fields.skip_bytes((ID_LEN + 1) as u64)?;
        let points = fields.read_u16()?.value;
        skip_items(fields, u32::from(points), CURVE_POINT_LEN)?;
    }
    Ok(())
}

pub(crate) fn checked_child_bytes(count: u32, available: u64) -> Result<u64, BankError> {
    let required = u64::from(count) * ID_LEN as u64;
    if required > available {
        return Err(child_count_error());
    }
    Ok(required)
}

pub(crate) fn read_children(
    fields: &mut SectionReader<'_>,
    count: u32,
) -> Result<Vec<u32>, BankError> {
    let capacity = usize::try_from(count).map_err(|_| child_count_error())?;
    let mut children = Vec::with_capacity(capacity);
    for _ in 0..count {
        children.push(fields.u32()?);
    }
    Ok(children)
}

const fn child_count_error() -> BankError {
    BankError::invalid(
        SectionId::HIRC,
        "a container lists more children than its body holds",
    )
}

pub(crate) fn skip_items<F: ObjectFields>(
    fields: &mut F,
    count: u32,
    len: usize,
) -> Result<(), F::Error> {
    fields.skip_bytes(bank_object_fields::item_bytes(count, len)? as u64)
}
