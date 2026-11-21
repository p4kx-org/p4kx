// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;
use std::io::{Read, Seek};

use p4kx_audio::{BankHircField, BankHircPlugin};
use schemars::JsonSchema;
use serde::ser::{Error as _, SerializeSeq};
use serde::{Serialize, Serializer};

use crate::hirc_view::node_source::state::InlineState;
use crate::source_value::SourcedValue;

use super::ObjectOrigins;

#[derive(Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
pub(in crate::hirc_view) enum PluginRow {
    BankData {
        ordinal: SourcedValue<u64>,
        index: SourcedValue<u8>,
        source_id: SourcedValue<u32>,
    },
    StateOverride {
        ordinal: SourcedValue<u64>,
        property_id: SourcedValue<u32>,
        accumulation: SourcedValue<u8>,
        value_bits: SourcedValue<u32>,
    },
}

pub(super) struct PluginRows<'view, 'input, R> {
    pub(super) reader: &'view RefCell<BankHircPlugin<'input, R>>,
    pub(super) origins: &'view ObjectOrigins,
}

impl<R: Read + Seek> Serialize for PluginRows<'_, '_, R> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut reader = self.reader.try_borrow_mut().map_err(S::Error::custom)?;
        let mut output = serializer.serialize_seq(None)?;
        if let Some(rows) = reader.bank_data() {
            for (ordinal, row) in rows.enumerate() {
                let row = row.map_err(S::Error::custom)?;
                output.serialize_element(&PluginRow::BankData {
                    ordinal: sourced_ordinal::<_, S::Error>(
                        self.origins,
                        ordinal,
                        &row.index,
                        "pluginBankDataOrdinal",
                    )?,
                    index: self
                        .origins
                        .field(&row.index, "pluginBankDataIndex")
                        .map_err(S::Error::custom)?,
                    source_id: self
                        .origins
                        .field(&row.source_id, "pluginBankDataSourceId")
                        .map_err(S::Error::custom)?,
                })?;
            }
        }
        if let Some(rows) = reader.overrides() {
            for (ordinal, row) in rows.enumerate() {
                let row = row.map_err(S::Error::custom)?;
                output.serialize_element(&PluginRow::StateOverride {
                    ordinal: sourced_ordinal::<_, S::Error>(
                        self.origins,
                        ordinal,
                        &row.property_id,
                        "pluginOverrideOrdinal",
                    )?,
                    property_id: self
                        .origins
                        .field(&row.property_id, "pluginOverridePropertyId")
                        .map_err(S::Error::custom)?,
                    accumulation: self
                        .origins
                        .field(&row.accumulation, "pluginOverrideAccumulation")
                        .map_err(S::Error::custom)?,
                    value_bits: self
                        .origins
                        .field(&row.value_bits, "pluginOverrideValueBits")
                        .map_err(S::Error::custom)?,
                })?;
            }
        }
        output.end()
    }
}

fn sourced_ordinal<T, E: serde::ser::Error>(
    origins: &ObjectOrigins,
    ordinal: usize,
    reader: &BankHircField<T>,
    label: &'static str,
) -> Result<SourcedValue<u64>, E> {
    let ordinal = u64::try_from(ordinal).map_err(E::custom)?;
    origins
        .derived(ordinal, &reader.range, label)
        .map_err(E::custom)
}

pub(super) struct InlineStateGroup<'view, 'input, R> {
    pub(super) reader: &'view RefCell<BankHircPlugin<'input, R>>,
    pub(super) origins: &'view ObjectOrigins,
}

impl<R: Read + Seek> Serialize for InlineStateGroup<'_, '_, R> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut reader = self.reader.try_borrow_mut().map_err(S::Error::custom)?;
        let rows = reader
            .states()
            .ok_or_else(|| S::Error::custom("the checked original State section is absent"))?;
        InlineState::read(self.origins, rows)
            .map_err(S::Error::custom)?
            .serialize(serializer)
    }
}
