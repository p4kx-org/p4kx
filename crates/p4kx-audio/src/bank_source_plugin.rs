// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};
use std::ops::Range;

use crate::bank_inline_state_rows::BankInlineStateRows;
use crate::bank_node_fx_slots::BankNodeFxSlots;
use crate::bank_object_fields::{ObjectFields, SeekObjectFields};
use crate::bank_rtpc_rows::BankRtpcRows;
use crate::bank_source_effect_metadata;
use crate::bank_structure::OBJECT_ID_LEN;
use crate::hirc_device::BankDeviceSlots;
use crate::hirc_effect::{self, BankEffectFields, BankEffectOverride};
use crate::hirc_plugin_family::{self, BankPluginKind};
use crate::hirc_plugin_parameters::{self, BankPluginDataReference};
use crate::read_at::SeekSource;
use crate::{BankError, BankHircObject, BankReadError};

/// Owner-bound original fields of an effect share set, custom effect or audio device.
pub struct BankHircPlugin<'input, R> {
    input: &'input mut SeekSource<R>,
    object: BankHircObject,
    kind: BankPluginKind,
    effect: BankEffectFields,
    slots: Option<BankDeviceSlots>,
    error: Option<BankError>,
}

impl<R> BankHircPlugin<'_, R> {
    /// Returns the original HIRC occurrence.
    #[must_use]
    pub const fn object(&self) -> &BankHircObject {
        &self.object
    }

    /// Returns which physical object kind owns the plug-in.
    #[must_use]
    pub const fn kind(&self) -> BankPluginKind {
        self.kind
    }

    /// Returns the checked effect baseline fields.
    #[must_use]
    pub const fn effect(&self) -> &BankEffectFields {
        &self.effect
    }

    /// Returns original effect slots on a device, including absent fields on a partial body.
    #[must_use]
    pub const fn device_slots(&self) -> Option<&BankDeviceSlots> {
        self.slots.as_ref()
    }

    /// Returns the format error for an incomplete or incorrectly bounded body.
    #[must_use]
    pub const fn format_error(&self) -> Option<&BankError> {
        self.error.as_ref()
    }

    /// Returns the checked original extent including the object ID.
    #[must_use]
    pub fn checked_range(&self) -> Range<u64> {
        let start = self.object.body_offset();
        let end = self
            .slots
            .as_ref()
            .and_then(BankDeviceSlots::checked_end)
            .or_else(|| self.effect.checked_end())
            .unwrap_or(start + OBJECT_ID_LEN as u64);
        start..end
    }

    /// Returns the unprojected original body suffix.
    #[must_use]
    pub fn unprojected_range(&self) -> Option<Range<u64>> {
        bank_source_effect_metadata::checked_suffix(&self.object, self.checked_range().end)
    }
}

impl<R: Read + Seek> BankHircPlugin<'_, R> {
    /// Replays checked bank-data references in original order.
    pub fn bank_data(&mut self) -> Option<BankPluginRows<'_, R, BankPluginDataReference>> {
        let range = self.effect.plugin.bank_data_range.as_ref()?;
        Some(BankPluginRows::new(
            self.input,
            range,
            hirc_plugin_parameters::read_reference,
        ))
    }

    /// Replays the complete physical state property override prefix.
    pub fn overrides(&mut self) -> Option<BankPluginRows<'_, R, BankEffectOverride>> {
        let range = self.effect.override_range.as_ref()?;
        Some(BankPluginRows::new(
            self.input,
            range,
            hirc_effect::read_override,
        ))
    }

    /// Replays the initial RTPC graph prefix using its original count and spans.
    pub fn rtpc(&mut self) -> Option<BankRtpcRows<'_, R>> {
        self.effect
            .rtpc
            .as_ref()
            .map(|extent| BankRtpcRows::new(self.input, extent))
    }

    /// Replays the original inline State section once its full layout fits.
    pub fn states(&mut self) -> Option<BankInlineStateRows<'_, R>> {
        let range = self.effect.states.clone()?;
        Some(BankInlineStateRows::new(self.input, range))
    }

    /// Replays original effect slots from the checked device list.
    pub fn effect_slots(&mut self) -> Option<BankNodeFxSlots<'_, R>> {
        let range = self.slots.as_ref()?.fields.slots_range.clone()?;
        Some(BankNodeFxSlots::new(self.input, range))
    }
}

/// A fixed-memory replay of original bank mappings or plug-in property overrides.
pub struct BankPluginRows<'input, R, Row> {
    fields: SeekObjectFields<'input, R>,
    read: RowReader<'input, R, Row>,
    failed: bool,
}

type RowReader<'input, R, Row> = fn(&mut SeekObjectFields<'input, R>) -> Result<Row, BankReadError>;

impl<'input, R, Row> BankPluginRows<'input, R, Row> {
    fn new(
        input: &'input mut SeekSource<R>,
        range: &Range<u64>,
        read: RowReader<'input, R, Row>,
    ) -> Self {
        Self {
            fields: SeekObjectFields::after_id(input, range.start, range.end - range.start, 0),
            read,
            failed: false,
        }
    }
}

impl<R: Read + Seek, Row> Iterator for BankPluginRows<'_, R, Row> {
    type Item = Result<Row, BankReadError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.failed || self.fields.remaining_bytes() == 0 {
            return None;
        }
        let row = (self.read)(&mut self.fields);
        self.failed = row.is_err();
        Some(row)
    }
}

pub(crate) fn observe<'input, R: Read + Seek>(
    input: &'input mut SeekSource<R>,
    stopped: &mut bool,
    object: BankHircObject,
    kind: BankPluginKind,
) -> Result<BankHircPlugin<'input, R>, BankReadError> {
    let mut effect = BankEffectFields::default();
    let mut slots = kind.device_slots();
    let error = bank_source_effect_metadata::observe(input, stopped, &object, |reader| {
        hirc_plugin_family::scan(reader, &mut effect, slots.as_mut())
    })?;
    Ok(BankHircPlugin {
        input,
        object,
        kind,
        effect,
        slots,
        error,
    })
}
