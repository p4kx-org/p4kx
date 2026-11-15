// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};

use p4kx_extraction::bank::{BankHircModulator, BankHircObject, BankNodePropertyKind};

use crate::audio_bank_inventory::VersionSite;
use crate::audio_bank_node_declaration_payload::NodeDeclaration;
use crate::audio_bank_node_origins::{NodeKind, NodeSites};
use crate::audio_bank_node_property_rows::{self, PropertyRows, PropertyScope};
use crate::audio_bank_origins::BankOrigins;
use crate::audio_bank_projection::BankProjection;
use crate::audio_bank_property_labels;
use crate::audio_bank_property_list;
use crate::audio_bank_rtpc;
use crate::audio_bank_rtpc_payload::RtpcOwner;
use crate::{MediaError, MediaWriter, Status, Subresource};

use super::payload::{MODULATOR, MetadataPayload, ObservedModulator};

pub(in crate::audio_bank_hirc) fn project<R: Read + Seek>(
    bank: &BankProjection<'_>,
    version: VersionSite,
    mut reader: BankHircModulator<'_, R>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let original = *reader.object();
    let sites = NodeSites::new(&original, version, NodeKind::Modulator);
    let mut origins = sites.record(bank.input, writer, None)?;
    let observed = observations(&sites, &mut origins, &reader, version.version)?;
    let error = reader.format_error().cloned();
    let (payload, status, issue) = super::declaration(
        version.version,
        error.as_ref(),
        observed,
        "audio.bank.invalid-hirc-modulator",
    );
    origins.finish().write(
        writer,
        &Subresource::new(MODULATOR, sites.indices().to_vec()),
        status,
        &MetadataPayload::Modulator(&payload),
        issue.as_slice(),
    )?;
    let projection = PropertyRows::new(bank, &sites, PropertyScope::Modulator);
    for kind in [
        BankNodePropertyKind::Properties,
        BankNodePropertyKind::RangedModifiers,
    ] {
        if let Some(rows) = reader.properties(kind) {
            audio_bank_node_property_rows::emit(&projection, kind, rows, writer)?;
        }
    }
    let rtpc = audio_bank_rtpc::node_rows(reader.rtpc(), error)?;
    audio_bank_rtpc::project(bank, version, &original, RtpcOwner::Modulator, rtpc, writer)
}

fn observations<R>(
    sites: &NodeSites,
    origins: &mut BankOrigins<'_, '_, '_>,
    reader: &BankHircModulator<'_, R>,
    version: u32,
) -> Result<ObservedModulator, MediaError> {
    let coverage = super::coverage(
        sites,
        origins,
        reader.object().kind(version),
        &reader.checked_range(),
        reader.unprojected_range().as_ref(),
        reader.format_error().is_some(),
    )?;
    let properties = &reader.fields().properties;
    Ok(ObservedModulator {
        coverage,
        properties: audio_bank_property_list::record(
            sites,
            origins,
            "observed/properties",
            properties.get(BankNodePropertyKind::Properties),
            &audio_bank_property_labels::MODULATOR,
        )?,
        ranged_modifiers: audio_bank_property_list::record(
            sites,
            origins,
            "observed/ranged_modifiers",
            properties.get(BankNodePropertyKind::RangedModifiers),
            &audio_bank_property_labels::MODULATOR,
        )?,
    })
}

pub(in crate::audio_bank_hirc) fn unsupported(
    bank: &BankProjection<'_>,
    version: VersionSite,
    original: &BankHircObject,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let sites = NodeSites::new(original, version, NodeKind::Modulator);
    let mut origins = sites.record(bank.input, writer, None)?;
    sites.summary(&mut origins, None, false)?;
    origins.finish().write(
        writer,
        &Subresource::new(MODULATOR, sites.indices().to_vec()),
        Status::Complete,
        &MetadataPayload::Modulator(&NodeDeclaration::Unsupported {
            version: version.version,
        }),
        &[],
    )
}
