// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};

use p4kx_extraction::bank::{
    BankAttenuationCurve, BankHircAttenuation, BankHircField, BankHircObject,
};

use self::payload::{
    AttenuationCurve, AttenuationPayload, AttenuationSelector, CURVE, POINT, SELECTOR, SUMMARY,
};
use crate::audio_bank_inventory::VersionSite;
use crate::audio_bank_node_declaration_payload::NodeDeclaration;
use crate::audio_bank_node_origins::{NodeKind, NodeSites};
use crate::audio_bank_projection::BankProjection;
use crate::audio_bank_rtpc;
use crate::audio_bank_rtpc_payload::RtpcOwner;
use crate::audio_bank_rtpc_rows;
use crate::{MediaError, MediaIssue, MediaWriter, Severity, Status, Subresource};

#[path = "audio_bank_attenuation_payload.rs"]
pub(crate) mod payload;
#[path = "audio_bank_attenuation_summary.rs"]
mod summary;

pub(crate) fn project<R: Read + Seek>(
    bank: &BankProjection<'_>,
    version: VersionSite,
    mut attenuation: BankHircAttenuation<'_, R>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let original = *attenuation.object();
    let sites = NodeSites::new(&original, version, NodeKind::Attenuation);
    let mut origins = sites.record(bank.input, writer, None)?;
    let observed = summary::observed(&attenuation, &sites, &mut origins)?;
    let (payload, status, issue) = if let Some(error) = attenuation.format_error() {
        let issue = MediaIssue::new(
            "audio.bank.invalid-attenuation",
            "HIRC",
            Severity::Error,
            error.to_string(),
        );
        (
            NodeDeclaration::Malformed {
                version: version.version,
                observed,
            },
            Status::Partial,
            Some(issue),
        )
    } else {
        (
            NodeDeclaration::Decoded {
                version: version.version,
                observed,
            },
            Status::Complete,
            None,
        )
    };
    let body = original.body_offset()..original.body_end_exclusive();
    sites.summary(&mut origins, Some(&body), issue.is_some())?;
    origins.finish().write(
        writer,
        &Subresource::new(SUMMARY, sites.indices().to_vec()),
        status,
        &AttenuationPayload::Summary(&payload),
        issue.as_slice(),
    )?;
    original_rows(bank, &sites, &mut attenuation, writer)?;
    let failure = attenuation.format_error().cloned();
    let rtpc = audio_bank_rtpc::node_rows(attenuation.rtpc(), failure)?;
    audio_bank_rtpc::project(
        bank,
        version,
        &original,
        RtpcOwner::Attenuation,
        rtpc,
        writer,
    )
}

pub(crate) fn unsupported(
    bank: &BankProjection<'_>,
    version: VersionSite,
    original: &BankHircObject,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let sites = NodeSites::new(original, version, NodeKind::Attenuation);
    let mut origins = sites.record(bank.input, writer, None)?;
    sites.summary(&mut origins, None, false)?;
    origins.finish().write(
        writer,
        &Subresource::new(SUMMARY, sites.indices().to_vec()),
        Status::Complete,
        &AttenuationPayload::Summary(&NodeDeclaration::Unsupported {
            version: version.version,
        }),
        &[],
    )
}

fn original_rows<R: Read + Seek>(
    bank: &BankProjection<'_>,
    sites: &NodeSites,
    attenuation: &mut BankHircAttenuation<'_, R>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    for (ordinal, value) in attenuation.fields().selectors.iter().enumerate() {
        selector(bank, sites, ordinal as u64, value, writer)?;
    }
    let [section, object] = sites.indices();
    for ordinal in 0..attenuation.fields().curves.len() {
        let original = attenuation.fields().curves.get(ordinal).ok_or_else(|| {
            MediaError::Contract("the selected attenuation curve is missing".into())
        })?;
        let curve_range = original.scaling.range.clone();
        curve(bank, sites, ordinal as u64, original, writer)?;
        let points = attenuation.points(ordinal).ok_or_else(|| {
            MediaError::Contract("the checked attenuation points are missing".into())
        })?;
        for (point_index, point) in points.enumerate() {
            let point = point.map_err(|cause| bank.replay_failure(cause))?;
            let mut origins = sites.record(bank.input, writer, Some(&point.from_bits.range))?;
            sites.index(
                &mut origins,
                2,
                &curve_range,
                "attenuationPointCurvePosition",
            )?;
            sites.index(
                &mut origins,
                3,
                &point.from_bits.range,
                "attenuationPointPosition",
            )?;
            let record = audio_bank_rtpc_rows::point_fields(sites, &mut origins, &point)?;
            origins.finish().write(
                writer,
                &Subresource::new(
                    POINT,
                    vec![section, object, ordinal as u64, point_index as u64],
                ),
                Status::Complete,
                &AttenuationPayload::Point(&record),
                &[],
            )?;
        }
    }
    Ok(())
}

fn selector(
    bank: &BankProjection<'_>,
    sites: &NodeSites,
    ordinal: u64,
    original: &BankHircField<i8>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let mut origins = sites.record(bank.input, writer, Some(&original.range))?;
    sites.index(
        &mut origins,
        2,
        &original.range,
        "attenuationSelectorPosition",
    )?;
    sites.field(
        &mut origins,
        "curve_index",
        &original.range,
        "attenuationSignedSelector",
    )?;
    let [section, object] = sites.indices();
    origins.finish().write(
        writer,
        &Subresource::new(SELECTOR, vec![section, object, ordinal]),
        Status::Complete,
        &AttenuationPayload::Selector(AttenuationSelector {
            curve_index: original.into(),
        }),
        &[],
    )
}

fn curve(
    bank: &BankProjection<'_>,
    sites: &NodeSites,
    ordinal: u64,
    original: &BankAttenuationCurve,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let mut origins = sites.record(bank.input, writer, Some(&original.scaling.range))?;
    sites.index(
        &mut origins,
        2,
        &original.scaling.range,
        "attenuationCurvePosition",
    )?;
    sites.field(
        &mut origins,
        "scaling",
        &original.scaling.range,
        "attenuationScaling",
    )?;
    sites.range(&mut origins, "points_range", &original.points_range)?;
    let point_count = sites.scalar(
        &mut origins,
        "point_count",
        original.point_count.as_ref(),
        "attenuationPointCount",
    )?;
    let [section, object] = sites.indices();
    origins.finish().write(
        writer,
        &Subresource::new(CURVE, vec![section, object, ordinal]),
        Status::Complete,
        &AttenuationPayload::Curve(&AttenuationCurve {
            scaling: (&original.scaling).into(),
            point_count,
            points_range: (&original.points_range).into(),
        }),
        &[],
    )
}
