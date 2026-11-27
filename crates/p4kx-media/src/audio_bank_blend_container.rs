// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};

use p4kx_extraction::bank::{
    BankBlendContainerFields, BankBlendLayers, BankHircContainerNode, BankHircObject,
};

use crate::audio_bank_blend_layer;
use crate::audio_bank_blend_origins;
use crate::audio_bank_blend_payload::{BlendObservations, BlendPayload, SUMMARY_NAMESPACE};
use crate::audio_bank_container_node;
use crate::audio_bank_inventory::VersionSite;
use crate::audio_bank_node_declarations::{Coverage, Declarations, Disposition};
use crate::audio_bank_node_origins::{NodeKind, NodeSites};
use crate::audio_bank_projection::BankProjection;
use crate::{MediaError, MediaWriter, Subresource};

pub(crate) fn project<R: Read + Seek>(
    projection: &BankProjection<'_>,
    version: VersionSite,
    container: &mut BankHircContainerNode<'_, R>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let original = *container.object();
    let declarations = Declarations::new(projection, &original, version);
    let disposition = audio_bank_container_node::disposition(container.status())?;
    summary(
        &declarations,
        disposition,
        container.fields().blend.as_ref(),
        writer,
    )?;
    if let Some(rows) = container.blend_layers() {
        layers(projection, version, &original, rows, writer)?;
    }
    Ok(())
}

pub(crate) fn unsupported(
    projection: &BankProjection<'_>,
    version: VersionSite,
    original: &BankHircObject,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let declarations = Declarations::new(projection, original, version);
    summary(&declarations, Disposition::Unsupported, None, writer)
}

fn summary(
    declarations: &Declarations<'_, '_>,
    disposition: Disposition<'_>,
    original: Option<&BankBlendContainerFields>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let sites = declarations.sites(NodeKind::BlendContainer);
    let mut prepared = sites.record(declarations.bank.input, writer, None)?;
    let observed = if let Some(fields) = original {
        BlendObservations {
            layers: audio_bank_blend_origins::table(
                &sites,
                &mut prepared,
                fields.layers.as_ref(),
                "observed/layers",
                "blendLayerCount",
            )?,
            continuous_validation: sites.scalar(
                &mut prepared,
                "observed/continuous_validation",
                fields.continuous_validation.as_ref(),
                "blendContinuousValidation",
            )?,
        }
    } else {
        BlendObservations::default()
    };
    let coverage = Coverage::read(
        original.map(|fields| fields.start),
        original.and_then(BankBlendContainerFields::complete_range),
    );
    let (record, status, issue) = declarations.summary(
        &sites,
        &mut prepared,
        disposition,
        &coverage,
        observed,
        "audio.bank.invalid-blend-container",
    )?;
    prepared.finish().write(
        writer,
        &Subresource::new(SUMMARY_NAMESPACE, sites.indices().to_vec()),
        status,
        &BlendPayload::Summary(&record),
        issue.as_slice(),
    )
}

fn layers<R: Read + Seek>(
    projection: &BankProjection<'_>,
    version: VersionSite,
    original: &BankHircObject,
    mut rows: BankBlendLayers<'_, R>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let sites = NodeSites::new(original, version, NodeKind::BlendContainer);
    while let Some(layer) = rows
        .next_layer()
        .map_err(|cause| projection.replay_failure(cause))?
    {
        audio_bank_blend_layer::project(projection, version, original, &sites, layer, writer)?;
    }
    Ok(())
}
