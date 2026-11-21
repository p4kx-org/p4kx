// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;
use std::io::{Read, Seek};

use p4kx_audio::{BankBusDuckRow, BankHircBus, BankObservedHeader};
use schemars::JsonSchema;
use serde::ser::{Error as _, SerializeSeq as _};
use serde::{Serialize, Serializer};

use super::NodeReader;
use super::ObjectOrigins;
use crate::hirc_view::StateRange;
use crate::source_evidence_error::SourceEvidenceError;
use crate::source_value::SourcedValue;

pub(super) type BusSchema = BusFields<Vec<DuckTarget>>;

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "WwiseBusSourceFields")]
pub(in crate::hirc_view) struct BusFields<Ducks> {
    #[serde(skip_serializing_if = "Option::is_none")]
    device_shareset_id: Option<SourcedValue<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    playback_flags: Option<SourcedValue<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_instances: Option<SourcedValue<u16>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    channel_config_bits: Option<SourcedValue<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    hdr_flags: Option<SourcedValue<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    recovery_time: Option<SourcedValue<i32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_duck_volume_bits: Option<SourcedValue<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    duck_count: Option<SourcedValue<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    duck_targets: Option<Ducks>,
    #[serde(skip_serializing_if = "Option::is_none")]
    raw_remainder: Option<SourcedValue<StateRange>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<SourcedValue<String>>,
}

impl<Ducks> BusFields<Ducks> {
    pub(super) fn read<R>(
        origins: &ObjectOrigins,
        header: &BankObservedHeader,
        bus: &BankHircBus<'_, R>,
        ducks: Ducks,
    ) -> Result<Self, SourceEvidenceError> {
        let original = bus.fields();
        let settings = &original.settings;
        let raw = bus.raw_remainder();
        Ok(Self {
            device_shareset_id: origins
                .optional(original.device_shareset_id.as_ref(), "busDeviceSharesetId")?,
            playback_flags: origins
                .optional(settings.playback_flags.as_ref(), "busPlaybackFlags")?,
            max_instances: origins.optional(settings.max_instances.as_ref(), "busMaxInstances")?,
            channel_config_bits: origins.optional(
                settings.channel_config_bits.as_ref(),
                "busChannelConfigBits",
            )?,
            hdr_flags: origins.optional(settings.hdr_flags.as_ref(), "busHdrFlags")?,
            recovery_time: origins.optional(original.recovery_time.as_ref(), "busRecoveryTime")?,
            max_duck_volume_bits: origins.optional(
                original.max_duck_volume_bits.as_ref(),
                "busMaxDuckVolumeBits",
            )?,
            duck_count: origins.optional(original.duck_count.as_ref(), "busDuckCount")?,
            duck_targets: original.duck_rows.is_some().then_some(ducks),
            raw_remainder: (!raw.is_empty())
                .then(|| origins.range(&raw, "busRawRemainder"))
                .transpose()?,
            error: bus
                .format_error()
                .map(|error| origins.coverage(error.to_string(), header, bus.object()))
                .transpose()?,
        })
    }
}

pub(super) struct DuckRows<'view, 'source, 'input, R> {
    pub(super) origins: &'view ObjectOrigins,
    pub(super) node: &'view RefCell<NodeReader<'source, 'input, R>>,
}

impl<R: Read + Seek> Serialize for DuckRows<'_, '_, '_, R> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut node = self.node.try_borrow_mut().map_err(S::Error::custom)?;
        let rows = node
            .ducks()
            .ok_or_else(|| S::Error::custom("the original duck list was not reached"))?;
        let mut sequence = serializer.serialize_seq(None)?;
        for raw in rows {
            let raw = raw.map_err(S::Error::custom)?;
            let observed = DuckTarget::read(self.origins, &raw).map_err(S::Error::custom)?;
            sequence.serialize_element(&observed)?;
        }
        sequence.end()
    }
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(in crate::hirc_view) struct DuckTarget {
    ordinal: SourcedValue<u32>,
    bus_id: SourcedValue<u32>,
    volume_bits: SourcedValue<u32>,
    fade_out_time: SourcedValue<i32>,
    fade_in_time: SourcedValue<i32>,
    fade_curve: SourcedValue<u8>,
    target_property: SourcedValue<u8>,
}

impl DuckTarget {
    fn read(origins: &ObjectOrigins, row: &BankBusDuckRow) -> Result<Self, SourceEvidenceError> {
        let extent = row.bus_id.range.start..row.target_property.range.end;
        Ok(Self {
            ordinal: origins.derived(row.ordinal, &extent, "busDuckOrdinal")?,
            bus_id: origins.field(&row.bus_id, "busDuckTargetId")?,
            volume_bits: origins.field(&row.volume_bits, "busDuckVolumeBits")?,
            fade_out_time: origins.field(&row.fade_out_time, "busDuckFadeOutTime")?,
            fade_in_time: origins.field(&row.fade_in_time, "busDuckFadeInTime")?,
            fade_curve: origins.field(&row.fade_curve, "busDuckFadeCurve")?,
            target_property: origins.field(&row.target_property, "busDuckTargetProperty")?,
        })
    }
}
