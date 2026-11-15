// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use crate::bank_object_fields::{BankHircField, ObjectFields};
use crate::hirc_node_aux::{self, BankNodeAuxFields};
use crate::hirc_node_fx::{self, BankNodeFxFields, BankNodeFxKind, BankNodeFxList};
use crate::hirc_node_positioning::{self, BankNodePositioningFields};
use crate::hirc_node_property::{self, BankNodePropertyExtent, BankNodePropertyKind};
use crate::hirc_rtpc::{self, BankRtpcExtent};
use crate::{BankError, SectionId, hirc_node_state};

const DUCK_WORD_COUNT: u64 = 4;
const DUCK_CODE_BYTES: u64 = 2;
pub(crate) const DUCK_LEN: u64 = DUCK_WORD_COUNT * size_of::<u32>() as u64 + DUCK_CODE_BYTES;

/// Checked fields and list extents of a version-154 Audio Bus or Auxiliary Bus.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct BankHircBusFields {
    /// Original output Override Bus ID.
    pub override_bus_id: Option<BankHircField<u32>>,
    /// Device `ShareSet` ID authored by a root bus.
    pub device_shareset_id: Option<BankHircField<u32>>,
    /// Original planar property IDs and raw value words.
    pub properties: Option<BankNodePropertyExtent>,
    /// Authored positioning fields and checked path extents.
    pub positioning: BankNodePositioningFields,
    /// Original auxiliary sends and reflection route.
    pub aux: BankNodeAuxFields,
    /// Bus instance-limit and routing settings.
    pub settings: BankHircBusSettings,
    /// Authored duck-recovery signed word.
    pub recovery_time: Option<BankHircField<i32>>,
    /// Raw binary32 maximum duck-volume word.
    pub max_duck_volume_bits: Option<BankHircField<u32>>,
    /// Authored number of duck target rows.
    pub duck_count: Option<BankHircField<u32>>,
    /// Complete checked duck rows in authored order.
    pub duck_rows: Option<Range<u64>>,
    /// Original RTPC count and checked curves.
    pub rtpc_chunk: Option<BankRtpcExtent>,
    /// Complete original inline State chunk.
    pub state_chunk: Option<Range<u64>>,
    effects: BankNodeFxFields,
    effect_count: Option<BankHircField<u8>>,
    metadata: BankNodeFxFields,
    metadata_count: Option<BankHircField<u8>>,
}

impl BankHircBusFields {
    /// Returns checked slots for the selected original list.
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
}

/// Original settings between the Aux declarations and ducking controls.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct BankHircBusSettings {
    /// Original playback rule flags.
    pub playback_flags: Option<BankHircField<u8>>,
    /// Original maximum instance count.
    pub max_instances: Option<BankHircField<u16>>,
    /// Original packed channel configuration.
    pub channel_config_bits: Option<BankHircField<u32>>,
    /// Original HDR routing flags.
    pub hdr_flags: Option<BankHircField<u8>>,
}

pub(crate) fn scan<F: ObjectFields>(
    input: &mut F,
    fields: &mut BankHircBusFields,
) -> Result<(), F::Error> {
    let output = input.read_u32()?;
    let root = output.value == 0;
    fields.override_bus_id = Some(output);
    if root {
        fields.device_shareset_id = Some(input.read_u32()?);
    }
    hirc_node_property::scan_list(
        input,
        BankNodePropertyKind::Properties,
        &mut fields.properties,
    )?;
    hirc_node_positioning::scan(input, &mut fields.positioning)?;
    hirc_node_aux::scan(input, &mut fields.aux)?;
    settings(input, &mut fields.settings)?;
    ducking(input, fields)?;
    hirc_node_fx::scan_owned(
        input,
        BankNodeFxKind::Effects,
        &mut fields.effects,
        &mut fields.effect_count,
    )?;
    hirc_node_fx::scan_owned(
        input,
        BankNodeFxKind::Metadata,
        &mut fields.metadata,
        &mut fields.metadata_count,
    )?;
    hirc_rtpc::scan(input, &mut fields.rtpc_chunk)?;
    fields.state_chunk = Some(hirc_node_state::skip(input)?);
    if input.remaining_bytes() > 0 {
        return Err(BankError::invalid(SectionId::HIRC, "bus layout has trailing bytes").into());
    }
    Ok(())
}

fn settings<F: ObjectFields>(
    input: &mut F,
    fields: &mut BankHircBusSettings,
) -> Result<(), F::Error> {
    fields.playback_flags = Some(input.read_u8()?);
    fields.max_instances = Some(input.read_u16()?);
    fields.channel_config_bits = Some(input.read_u32()?);
    fields.hdr_flags = Some(input.read_u8()?);
    Ok(())
}

fn ducking<F: ObjectFields>(input: &mut F, fields: &mut BankHircBusFields) -> Result<(), F::Error> {
    fields.recovery_time = Some(input.scalar(i32::from_le_bytes)?);
    fields.max_duck_volume_bits = Some(input.read_u32()?);
    let count = input.read_u32()?;
    let needed = u64::from(count.value) * DUCK_LEN;
    fields.duck_count = Some(count);
    let start = input.offset();
    let checked = input.remaining_bytes().min(needed) / DUCK_LEN * DUCK_LEN;
    fields.duck_rows = Some(start..start + checked);
    input.skip_bytes(checked)?;
    input.skip_bytes(needed - checked)
}
