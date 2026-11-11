// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};

use p4kx_extraction::bank::{BankBlendPoint, BankBlendPointRows, BankHircField};

use crate::audio_bank_blend_origins::{self, Occurrence};
use crate::audio_bank_blend_payload::{
    BlendFloatWord, BlendPayload, BlendPointRecord, POINT_NAMESPACE,
};
use crate::audio_bank_node_origins::NodeSites;
use crate::audio_bank_origins::BankOrigins;
use crate::audio_bank_projection::BankProjection;
use crate::{MediaError, MediaIssue, MediaWriter, Severity, Subresource};

pub(crate) fn project<R: Read + Seek>(
    projection: &BankProjection<'_>,
    sites: &NodeSites,
    layer: Occurrence,
    association: Occurrence,
    rows: BankBlendPointRows<'_, R>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    for row in rows {
        let row = row.map_err(|cause| projection.replay_failure(cause))?;
        emit(projection, sites, layer, association, &row, writer)?;
    }
    Ok(())
}

fn emit(
    projection: &BankProjection<'_>,
    sites: &NodeSites,
    layer: Occurrence,
    association: Occurrence,
    row: &BankBlendPoint,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let range = &row.from_bits.range;
    let mut prepared = sites.record(projection.input, writer, Some(range))?;
    sites.index(&mut prepared, 2, &layer.range(), "blendLayerPosition")?;
    sites.index(
        &mut prepared,
        3,
        &association.range(),
        "blendAssociationPosition",
    )?;
    sites.index(&mut prepared, 4, range, "blendPointPosition")?;
    let record = read(sites, &mut prepared, row)?;
    let malformed = matches!(record, BlendPointRecord::Malformed { .. });
    let status =
        audio_bank_blend_origins::layout(sites, &mut prepared, &row.checked_range, malformed)?;
    let issue = malformed.then(|| {
        MediaIssue::new(
            "audio.bank.truncated-blend-point",
            "HIRC",
            Severity::Error,
            "the original crossfade point ends before all its words fit",
        )
    });
    let [section, object] = sites.indices();
    prepared.finish().write(
        writer,
        &Subresource::new(
            POINT_NAMESPACE,
            vec![
                section,
                object,
                layer.ordinal,
                association.ordinal,
                u64::from(row.ordinal),
            ],
        ),
        status,
        &BlendPayload::Point(&record),
        issue.as_slice(),
    )
}

fn read(
    sites: &NodeSites,
    prepared: &mut BankOrigins<'_, '_, '_>,
    row: &BankBlendPoint,
) -> Result<BlendPointRecord, MediaError> {
    let from = word(
        sites,
        prepared,
        &row.from_bits,
        "from",
        "blendPointFromBits",
    )?;
    let to = row
        .to_bits
        .as_ref()
        .map(|word_bits| word(sites, prepared, word_bits, "to", "blendPointToBits"))
        .transpose()?;
    let interpolation = sites.scalar(
        prepared,
        "interpolation",
        row.interpolation.as_ref(),
        "blendPointInterpolation",
    )?;
    match (to, interpolation) {
        (Some(to), Some(interpolation)) => Ok(BlendPointRecord::Decoded {
            from,
            to,
            interpolation,
        }),
        (to, None) => Ok(BlendPointRecord::Malformed { from, to }),
        _ => Err(MediaError::Contract(
            "Blend point interpolation lacks its preceding word".into(),
        )),
    }
}

fn word(
    sites: &NodeSites,
    prepared: &mut BankOrigins<'_, '_, '_>,
    original: &BankHircField<u32>,
    name: &str,
    label: &'static str,
) -> Result<BlendFloatWord, MediaError> {
    sites.field(prepared, &format!("{name}/bits"), &original.range, label)?;
    let value = f32::from_bits(original.value);
    let finite_value = value.is_finite().then_some(value);
    if finite_value.is_some() {
        sites.converted(
            prepared,
            &format!("{name}/finite_value"),
            &original.range,
            "blendFinitePointValue",
        )?;
    }
    Ok(BlendFloatWord {
        bits: original.into(),
        finite_value,
    })
}
