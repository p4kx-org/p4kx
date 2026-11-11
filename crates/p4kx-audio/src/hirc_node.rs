// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use crate::bank_object_fields::{self, BankHircField, ObjectFields};
use crate::bank_reader::SectionReader;
use crate::hirc_node_aux::{self, BankNodeAuxFields};
use crate::hirc_node_fx::{self, BankNodeFxFields, BankNodeFxKind, BankNodeFxList};
use crate::hirc_node_parameters::BankNodeParameters;
use crate::hirc_node_playback::{self, BankNodePlaybackFields};
use crate::hirc_node_positioning;
use crate::hirc_node_property::{self, BankNodePropertyExtents};
use crate::hirc_node_state;
use crate::hirc_rtpc::{self, BankRtpcExtent};

pub(crate) use crate::hirc_rtpc::CURVE_POINT_LEN;
use crate::{BankError, SectionId};

pub(crate) const ID_LEN: usize = 4;

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
    /// Original initial effect declarations.
    pub effects: BankNodeFxFields,
    /// Original metadata effect count when readable.
    pub metadata_count: Option<BankHircField<u8>>,
    /// Original initial metadata effect declarations.
    pub metadata: BankNodeFxFields,
    /// Original override bus ID immediately before the parent ID.
    pub pre_parent_word: Option<BankHircField<u32>>,
    /// Original parent ID, including zero, when readable.
    pub parent_id: Option<BankHircField<u32>>,
}

impl BankNodeParentFields {
    /// Returns the original effect or metadata fields and checked extent.
    #[must_use]
    pub const fn fx(&self, kind: BankNodeFxKind) -> BankNodeFxList<'_> {
        match kind {
            BankNodeFxKind::Effects => BankNodeFxList {
                declared_count: self.effect_count.as_ref(),
                fields: &self.effects,
            },
            BankNodeFxKind::Metadata => BankNodeFxList {
                declared_count: self.metadata_count.as_ref(),
                fields: &self.metadata,
            },
        }
    }

    /// Returns the original override bus ID, including explicit zero.
    #[must_use]
    pub const fn override_bus_id(&self) -> Option<&BankHircField<u32>> {
        self.pre_parent_word.as_ref()
    }
}

pub(crate) fn read_base<F: ObjectFields>(
    fields: &mut F,
    observed: &mut BankNodeParentFields,
) -> Result<u32, F::Error> {
    read_base_observed(
        fields,
        observed,
        &mut BankNodePropertyExtents::default(),
        &mut BankNodeAuxFields::default(),
        &mut BankNodePlaybackFields::default(),
        &mut None,
        &mut None,
    )
}

pub(crate) fn read_base_parameters<F: ObjectFields>(
    fields: &mut F,
    parent: &mut BankNodeParentFields,
    parameters: &mut BankNodeParameters,
) -> Result<u32, F::Error> {
    read_base_observed(
        fields,
        parent,
        &mut parameters.properties,
        &mut parameters.aux,
        &mut parameters.playback,
        &mut parameters.state_chunk,
        &mut parameters.rtpc_chunk,
    )
}

pub(crate) fn read_base_observed<F: ObjectFields>(
    fields: &mut F,
    observed: &mut BankNodeParentFields,
    properties: &mut BankNodePropertyExtents,
    aux: &mut BankNodeAuxFields,
    playback: &mut BankNodePlaybackFields,
    state: &mut Option<Range<u64>>,
    rtpc: &mut Option<BankRtpcExtent>,
) -> Result<u32, F::Error> {
    let parent = read_parent(fields, observed)?;
    fields.read_array::<1>()?;
    hirc_node_property::scan(fields, properties)?;
    hirc_node_positioning::scan(fields, &mut playback.positioning)?;
    hirc_node_aux::scan(fields, aux)?;
    hirc_node_playback::advanced(fields, &mut playback.advanced)?;
    *state = Some(hirc_node_state::skip(fields)?);
    hirc_rtpc::scan(fields, rtpc)?;
    Ok(parent)
}

fn read_parent<F: ObjectFields>(
    fields: &mut F,
    observed: &mut BankNodeParentFields,
) -> Result<u32, F::Error> {
    hirc_node_fx::scan(
        fields,
        BankNodeFxKind::Effects,
        &mut observed.effects,
        &mut observed.effect_count,
    )?;
    hirc_node_fx::scan(
        fields,
        BankNodeFxKind::Metadata,
        &mut observed.metadata,
        &mut observed.metadata_count,
    )?;
    observed.pre_parent_word = Some(fields.read_u32()?);
    let parent = fields.read_u32()?;
    let id = parent.value;
    observed.parent_id = Some(parent);
    Ok(id)
}

pub(crate) fn skip_rtpcs<F: ObjectFields>(fields: &mut F) -> Result<(), F::Error> {
    hirc_rtpc::scan(fields, &mut None)
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
