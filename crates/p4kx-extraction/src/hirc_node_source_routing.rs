// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};

use p4kx_audio::{BankNodeAdvancedFields, BankNodeFxKind, BankNodeFxSlot};
use schemars::JsonSchema;
use serde::Serialize;

use crate::source_evidence_error::SourceEvidenceError;
use crate::source_value::SourcedValue;

use super::super::StateRange;
use super::ObjectOrigins;
use super::reader::NodeReader;

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct NodeFx {
    #[serde(skip_serializing_if = "Option::is_none")]
    override_parent_bits: Option<SourcedValue<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    declared_count: Option<SourcedValue<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bypass_all_bits: Option<SourcedValue<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    complete_range: Option<SourcedValue<StateRange>>,
    slots: Vec<NodeFxSlot>,
}

impl NodeFx {
    pub(super) fn read<R: Read + Seek, E: serde::ser::Error>(
        origins: &ObjectOrigins,
        node: &mut NodeReader<'_, '_, R>,
        kind: BankNodeFxKind,
    ) -> Result<Option<Self>, E> {
        let Some(list) = node.parent().map(|parent| parent.fx(kind)) else {
            return Ok(None);
        };
        if list.fields.start.is_none() {
            return Ok(None);
        }
        let labels = match kind {
            BankNodeFxKind::Effects => ("nodeFxOverrideParentBits", "nodeFxCount", "nodeFxExtent"),
            BankNodeFxKind::Metadata => (
                "nodeMetadataOverrideParentBits",
                "nodeMetadataCount",
                "nodeMetadataExtent",
            ),
            _ => return Err(E::custom("unexpected original node effect layout")),
        };
        let mut output = Self {
            override_parent_bits: origins
                .optional(list.fields.override_parent_bits.as_ref(), labels.0)
                .map_err(E::custom)?,
            declared_count: origins
                .optional(list.declared_count, labels.1)
                .map_err(E::custom)?,
            bypass_all_bits: origins
                .optional(list.fields.bypass_all_bits.as_ref(), "nodeFxBypassAllBits")
                .map_err(E::custom)?,
            complete_range: list
                .complete_range()
                .map(|range| origins.range(&range, labels.2))
                .transpose()
                .map_err(E::custom)?,
            slots: Vec::new(),
        };
        if let Some(rows) = node.fx_slots(kind) {
            for row in rows {
                let slot = row.map_err(E::custom)?;
                output
                    .slots
                    .push(NodeFxSlot::read(origins, &slot).map_err(E::custom)?);
            }
        }
        Ok(Some(output))
    }
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct NodeFxSlot {
    ordinal: SourcedValue<u8>,
    range: SourcedValue<StateRange>,
    slot_index: SourcedValue<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    effect_id: Option<SourcedValue<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    slot_bits: Option<SourcedValue<u8>>,
}

impl NodeFxSlot {
    fn read(origins: &ObjectOrigins, slot: &BankNodeFxSlot) -> Result<Self, SourceEvidenceError> {
        Ok(Self {
            ordinal: origins.derived(slot.ordinal, &slot.index.range, "nodeFxSlotPosition")?,
            range: origins.range(&slot.checked_range, "nodeFxSlotExtent")?,
            slot_index: origins.field(&slot.index, "nodeFxSlotIndex")?,
            effect_id: origins.optional(slot.effect_id.as_ref(), "nodeFxReferenceId")?,
            slot_bits: origins.optional(slot.bits.as_ref(), "nodeFxSlotBits")?,
        })
    }
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct NodeAux {
    #[serde(skip_serializing_if = "Option::is_none")]
    flags: Option<SourcedValue<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reflections_bus_id: Option<SourcedValue<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    user_bus_ids: Option<Vec<Option<SourcedValue<u32>>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    complete_range: Option<SourcedValue<StateRange>>,
}

impl NodeAux {
    pub(super) fn read<R: Read + Seek, E: serde::ser::Error>(
        origins: &ObjectOrigins,
        node: &mut NodeReader<'_, '_, R>,
    ) -> Result<Option<Self>, E> {
        let Some(fields) = node.aux().filter(|fields| fields.start.is_some()) else {
            return Ok(None);
        };
        let flags = origins
            .optional(fields.flags.as_ref(), "nodeAuxFlags")
            .map_err(E::custom)?;
        let reflections_bus_id = origins
            .optional(
                fields.reflections_bus_id.as_ref(),
                "nodeReflectionsAuxBusId",
            )
            .map_err(E::custom)?;
        let complete_range = fields
            .complete_range()
            .map(|range| origins.range(&range, "nodeAuxExtent"))
            .transpose()
            .map_err(E::custom)?;
        let user_bus_ids = node
            .user_aux_buses()
            .map_err(E::custom)?
            .map(|buses| {
                buses
                    .iter()
                    .map(|bus| origins.optional(bus.as_ref(), "nodeUserAuxBusId"))
                    .collect::<Result<_, _>>()
            })
            .transpose()
            .map_err(E::custom)?;
        Ok(Some(Self {
            flags,
            reflections_bus_id,
            user_bus_ids,
            complete_range,
        }))
    }
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct NodeAdvancedSettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    flags: Option<SourcedValue<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    virtual_queue_behavior: Option<SourcedValue<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_instances: Option<SourcedValue<u16>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    below_threshold_behavior: Option<SourcedValue<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    behavior_flags: Option<SourcedValue<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    complete_range: Option<SourcedValue<StateRange>>,
}

impl NodeAdvancedSettings {
    pub(super) fn read(
        origins: &ObjectOrigins,
        fields: Option<&BankNodeAdvancedFields>,
    ) -> Result<Option<Self>, SourceEvidenceError> {
        let Some(fields) = fields.filter(|fields| fields.start.is_some()) else {
            return Ok(None);
        };
        Ok(Some(Self {
            flags: origins.optional(fields.flags.as_ref(), "nodeAdvancedFlags")?,
            virtual_queue_behavior: origins.optional(
                fields.virtual_queue_behavior.as_ref(),
                "nodeAdvancedVirtualQueueBehavior",
            )?,
            max_instances: origins
                .optional(fields.max_instances.as_ref(), "nodeAdvancedMaxInstances")?,
            below_threshold_behavior: origins.optional(
                fields.below_threshold_behavior.as_ref(),
                "nodeAdvancedBelowThresholdBehavior",
            )?,
            behavior_flags: origins
                .optional(fields.behavior_flags.as_ref(), "nodeAdvancedBehaviorFlags")?,
            complete_range: fields
                .complete_range()
                .map(|range| origins.range(&range, "nodeAdvancedExtent"))
                .transpose()?,
        }))
    }
}
