// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};

use p4kx_extraction::bank::{
    BankBlendAssociation, BankBlendAssociationFields, BankBlendAssociations,
};

use crate::audio_bank_blend_origins::{self, Occurrence};
use crate::audio_bank_blend_payload::{
    ASSOCIATION_NAMESPACE, BlendAssociationRecord, BlendPayload,
};
use crate::audio_bank_blend_points;
use crate::audio_bank_node_declaration_payload::NodeReferenceStatus;
use crate::audio_bank_node_origins::NodeSites;
use crate::audio_bank_origins::BankOrigins;
use crate::audio_bank_projection::BankProjection;
use crate::{MediaError, MediaIssue, MediaWriter, Severity, Subresource};

pub(crate) fn project<R: Read + Seek>(
    projection: &BankProjection<'_>,
    sites: &NodeSites,
    layer: Occurrence,
    mut rows: BankBlendAssociations<'_, R>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    while let Some(mut association) = rows
        .next_association()
        .map_err(|cause| projection.replay_failure(cause))?
    {
        emit(projection, sites, layer, &association, writer)?;
        let occurrence = Occurrence::new(association.ordinal, &association.fields.child_id.range);
        if let Some(points) = association.points() {
            audio_bank_blend_points::project(projection, sites, layer, occurrence, points, writer)?;
        }
    }
    Ok(())
}

fn emit<R>(
    projection: &BankProjection<'_>,
    sites: &NodeSites,
    layer: Occurrence,
    row: &BankBlendAssociation<'_, R>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let range = &row.fields.child_id.range;
    let mut prepared = sites.record(projection.input, writer, Some(range))?;
    sites.index(&mut prepared, 2, &layer.range(), "blendLayerPosition")?;
    sites.index(&mut prepared, 3, range, "blendAssociationPosition")?;
    let status = audio_bank_blend_origins::layout(
        sites,
        &mut prepared,
        &row.checked_range,
        row.issue.is_some(),
    )?;
    let record = read(sites, &mut prepared, &row.fields, row.issue.is_some())?;
    let issue = row.issue.as_ref().map(|error| {
        MediaIssue::new(
            "audio.bank.truncated-blend-association",
            "HIRC",
            Severity::Error,
            error.to_string(),
        )
    });
    let [section, object] = sites.indices();
    prepared.finish().write(
        writer,
        &Subresource::new(
            ASSOCIATION_NAMESPACE,
            vec![section, object, layer.ordinal, u64::from(row.ordinal)],
        ),
        status,
        &BlendPayload::Association(&record),
        issue.as_slice(),
    )
}

fn read(
    sites: &NodeSites,
    prepared: &mut BankOrigins<'_, '_, '_>,
    fields: &BankBlendAssociationFields,
    malformed: bool,
) -> Result<BlendAssociationRecord, MediaError> {
    let range = &fields.child_id.range;
    sites.field(prepared, "child_id", range, "blendAssociatedChildId")?;
    sites.converted(
        prepared,
        "reference_resolution",
        range,
        "blendReferenceUnchecked",
    )?;
    let child_id = (&fields.child_id).into();
    let points = audio_bank_blend_origins::table(
        sites,
        prepared,
        fields.points.as_ref(),
        "points",
        "blendPointCount",
    )?;
    let reference_resolution = NodeReferenceStatus::Unchecked;
    let record = match (malformed, points) {
        (false, Some(points)) => BlendAssociationRecord::Decoded {
            child_id,
            points,
            reference_resolution,
        },
        (true, points) => BlendAssociationRecord::Malformed {
            child_id,
            points,
            reference_resolution,
        },
        _ => {
            return Err(MediaError::Contract(
                "complete Blend association lacks its point extent".into(),
            ));
        }
    };
    Ok(record)
}
