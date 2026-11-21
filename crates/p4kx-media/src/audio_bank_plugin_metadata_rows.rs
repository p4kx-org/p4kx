// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};
use std::ops::Range;

use p4kx_extraction::bank::{BankEffectOverride, BankHircPlugin, BankPluginDataReference};

use crate::audio_bank_node_origins::NodeSites;
use crate::audio_bank_projection::BankProjection;
use crate::{MediaError, MediaWriter, Status, Subresource};

use super::super::payload::{
    BANK_DATA, MetadataPayload, OVERRIDE, PluginBankData, PluginStateOverride,
};

pub(super) fn project<R: Read + Seek>(
    bank: &BankProjection<'_>,
    sites: &NodeSites,
    reader: &mut BankHircPlugin<'_, R>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    if let Some(rows) = reader.bank_data() {
        for (ordinal, row) in rows.enumerate() {
            bank_reference(
                bank,
                sites,
                ordinal as u64,
                &row.map_err(|cause| bank.replay_failure(cause))?,
                writer,
            )?;
        }
    }
    if let Some(rows) = reader.overrides() {
        for (ordinal, row) in rows.enumerate() {
            state_override(
                bank,
                sites,
                ordinal as u64,
                &row.map_err(|cause| bank.replay_failure(cause))?,
                writer,
            )?;
        }
    }
    Ok(())
}

fn bank_reference(
    bank: &BankProjection<'_>,
    sites: &NodeSites,
    ordinal: u64,
    row: &BankPluginDataReference,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let mut origins = sites.record(bank.input, writer, Some(&row.index.range))?;
    sites.index(&mut origins, 2, &row.index.range, "pluginBankDataOrdinal")?;
    sites.field(
        &mut origins,
        "index",
        &row.index.range,
        "pluginBankDataIndex",
    )?;
    sites.field(
        &mut origins,
        "source_id",
        &row.source_id.range,
        "pluginBankDataSourceId",
    )?;
    origins.finish().write(
        writer,
        &subresource(sites, BANK_DATA, ordinal),
        Status::Complete,
        &MetadataPayload::BankData(PluginBankData {
            index: (&row.index).into(),
            source_id: (&row.source_id).into(),
        }),
        &[],
    )
}

fn state_override(
    bank: &BankProjection<'_>,
    sites: &NodeSites,
    ordinal: u64,
    row: &BankEffectOverride,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let mut origins = sites.record(bank.input, writer, Some(&row.property_id.range))?;
    sites.index(
        &mut origins,
        2,
        &row.property_id.range,
        "pluginOverrideOrdinal",
    )?;
    for (path, label, range) in override_origins(row) {
        sites.field(&mut origins, path, range, label)?;
    }
    origins.finish().write(
        writer,
        &subresource(sites, OVERRIDE, ordinal),
        Status::Complete,
        &MetadataPayload::StateOverride(PluginStateOverride {
            property_id: (&row.property_id).into(),
            accumulation: (&row.accumulation).into(),
            value_bits: (&row.value_bits).into(),
        }),
        &[],
    )
}

const fn override_origins(
    row: &BankEffectOverride,
) -> [(&'static str, &'static str, &Range<u64>); 3] {
    [
        (
            "property_id",
            "pluginOverridePropertyId",
            &row.property_id.range,
        ),
        (
            "accumulation",
            "pluginOverrideAccumulation",
            &row.accumulation.range,
        ),
        (
            "value_bits",
            "pluginOverrideValueBits",
            &row.value_bits.range,
        ),
    ]
}

fn subresource(sites: &NodeSites, namespace: &'static str, ordinal: u64) -> Subresource {
    let [section, object] = sites.indices();
    Subresource::new(namespace, vec![section, object, ordinal])
}
