// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use crate::bank_object_fields::{BankHircField, ObjectFields};
use crate::bank_reader::SectionReader;
use crate::hirc_node_aux::{self, BankNodeAuxFields};
use crate::hirc_node_fx::{self, BankNodeFxFields, BankNodeFxKind, BankNodeFxList};
use crate::hirc_node_parameters::BankNodeParameters;
use crate::hirc_node_playback::{self, BankNodePlaybackFields};
use crate::hirc_node_positioning;
use crate::hirc_node_property::{self, BankNodePropertyExtents};
use crate::hirc_node_state;
use crate::hirc_rtpc::{self, BankRtpcExtent};

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
    /// The checked Random/Sequence loop words differ from one play with zero modifiers.
    pub has_authored_loops: bool,
    /// The original Switch defines membership groups or a nonzero default value.
    pub has_switch_conditions: bool,
    pub(crate) evidence: ContainerEvidence,
    pub(crate) child_count: Option<BankHircField<u32>>,
    pub(crate) random_sequence_mode: Option<BankHircField<u8>>,
}

impl Container {
    /// Returns the original checked child count and its bank-relative range.
    #[must_use]
    pub const fn child_count_evidence(&self) -> Option<&BankHircField<u32>> {
        self.child_count.as_ref()
    }

    /// Returns the bank-relative range of the child ID at its original physical ordinal.
    #[must_use]
    pub fn child_id_byte_range(&self, ordinal: usize) -> Option<Range<u64>> {
        child_id_byte_range(self.child_count_evidence()?, ordinal)
    }

    /// Returns the original Random/Sequence mode byte, including zero or an unknown mode.
    #[must_use]
    pub const fn random_sequence_mode_evidence(&self) -> Option<&BankHircField<u8>> {
        self.random_sequence_mode.as_ref()
    }

    /// Returns whether the checked Random/Sequence transition mode is nonzero.
    #[must_use]
    pub const fn has_authored_transitions(&self) -> bool {
        self.transition_mode_evidence().is_some()
    }

    /// Returns whether an original Blend child association contains checked crossfade points.
    #[must_use]
    pub const fn has_blend_curves(&self) -> bool {
        self.blend_point_count_evidence().is_some()
    }

    /// Returns the nonzero Random/Sequence transition mode and its bank-relative range.
    #[must_use]
    pub const fn transition_mode_evidence(&self) -> Option<&BankHircField<u8>> {
        if let ContainerEvidence::RandomSequence {
            transition_mode, ..
        } = &self.evidence
        {
            transition_mode.as_ref()
        } else {
            None
        }
    }

    /// Returns the first nonempty checked Blend curve's point count in physical order.
    #[must_use]
    pub const fn blend_point_count_evidence(&self) -> Option<&BankHircField<u32>> {
        if let ContainerEvidence::Blend(count) = &self.evidence {
            count.as_ref()
        } else {
            None
        }
    }

    /// Returns the first non-neutral original Random/Sequence loop word.
    #[must_use]
    pub const fn loop_evidence(&self) -> Option<&ContainerLoopEvidence> {
        self.evidence.loop_evidence()
    }

    /// Returns the original nonzero Switch default or the checked nonempty group count.
    #[must_use]
    pub const fn switch_condition_evidence(&self) -> Option<&ContainerSwitchEvidence> {
        self.evidence.switch_condition_evidence()
    }
}

/// The original Random/Sequence loop word that establishes authored loops.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ContainerLoopEvidence {
    /// Original count differs from one.
    Count(BankHircField<u16>),
    /// Original minimum modifier is nonzero.
    Minimum(BankHircField<u16>),
    /// Original maximum modifier is nonzero.
    Maximum(BankHircField<u16>),
}

/// The original Switch word that establishes an authored condition.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ContainerSwitchEvidence {
    /// Original default value ID is nonzero.
    DefaultValue(BankHircField<u32>),
    /// Original membership group count is positive and its full table fits.
    MembershipGroupCount(BankHircField<u32>),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum ContainerEvidence {
    None,
    RandomSequence {
        transition_mode: Option<BankHircField<u8>>,
        loop_field: Option<ContainerLoopEvidence>,
    },
    Switch(Option<ContainerSwitchEvidence>),
    Blend(Option<BankHircField<u32>>),
}

impl ContainerEvidence {
    pub(crate) const fn loop_evidence(&self) -> Option<&ContainerLoopEvidence> {
        if let Self::RandomSequence { loop_field, .. } = self {
            loop_field.as_ref()
        } else {
            None
        }
    }

    pub(crate) const fn switch_condition_evidence(&self) -> Option<&ContainerSwitchEvidence> {
        if let Self::Switch(field) = self {
            field.as_ref()
        } else {
            None
        }
    }
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
    /// Original packed priority and MIDI behavior flags immediately after the parent.
    pub priority_flags: Option<BankHircField<u8>>,
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
    observed.priority_flags = Some(fields.read_u8()?);
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

pub(crate) fn child_id_byte_range(
    count: &BankHircField<u32>,
    ordinal: usize,
) -> Option<Range<u64>> {
    let index = u32::try_from(ordinal)
        .ok()
        .filter(|&index| index < count.value)?;
    let start = count.range.end + u64::from(index) * ID_LEN as u64;
    Some(start..start + ID_LEN as u64)
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
