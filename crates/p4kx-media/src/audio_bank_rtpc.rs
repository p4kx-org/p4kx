// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};
use std::ops::Range;

use p4kx_extraction::BankError;
use p4kx_extraction::bank::{BankHircObject, BankRtpcRead, BankRtpcRows};

use crate::audio_bank_inventory::VersionSite;
use crate::audio_bank_node_origins::{NodeKind, NodeSites};
use crate::audio_bank_origins::BankOrigins;
use crate::audio_bank_projection::BankProjection;
use crate::audio_bank_rtpc_payload::{RtpcOwner, RtpcRecord};
use crate::audio_bank_rtpc_rows;
use crate::{MediaError, MediaIssue, MediaWriter, Severity, Status, Subresource};

pub(crate) fn node_rows<R>(
    rows: Option<BankRtpcRows<'_, R>>,
    error: Option<BankError>,
) -> Result<BankRtpcRead<'_, R>, MediaError> {
    match (rows, error) {
        (Some(rows), _) if rows.checked_curve_count() == rows.declared_curve_count().value => {
            Ok(BankRtpcRead::Readable(rows))
        }
        (Some(rows), Some(error)) => Ok(BankRtpcRead::Malformed { rows, error }),
        (None, Some(error)) => Ok(BankRtpcRead::Unavailable(error)),
        _ => Err(MediaError::Contract(
            "inconsistent shared node RTPC observation".into(),
        )),
    }
}

pub(crate) fn project<R: Read + Seek>(
    projection: &BankProjection<'_>,
    version: VersionSite,
    original: &BankHircObject,
    owner: RtpcOwner,
    read: BankRtpcRead<'_, R>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let sites = NodeSites::new(original, version, NodeKind::Rtpc);
    let body = match owner {
        RtpcOwner::BlendLayer(layer) => layer.checked_range(),
        _ => original.body_offset()..original.body_end_exclusive(),
    };
    let mut prepared = sites.record(projection.input, writer, None)?;
    capture_layer(owner, &sites, &mut prepared)?;
    let observed = observe(&sites, &mut prepared, &body, version.version, read)?;
    let status = if observed.issue.is_some() {
        Status::Partial
    } else {
        Status::Complete
    };
    prepared.finish().write(
        writer,
        &Subresource::new(
            owner.summary_namespace(),
            owner.indices(sites.indices(), &[]),
        ),
        status,
        &owner.summary(&observed.record),
        observed.issue.as_slice(),
    )?;
    if let Some(rows) = observed.rows {
        audio_bank_rtpc_rows::project(projection, &sites, owner, rows, writer)?;
    }
    Ok(())
}

pub(super) fn capture_layer(
    owner: RtpcOwner,
    sites: &NodeSites,
    prepared: &mut BankOrigins<'_, '_, '_>,
) -> Result<(), MediaError> {
    if let RtpcOwner::BlendLayer(layer) = owner {
        sites.index(prepared, 2, &layer.id_range(), "blendLayerPosition")?;
    }
    Ok(())
}

struct ObservedRtpc<'input, R> {
    record: RtpcRecord,
    issue: Option<MediaIssue>,
    rows: Option<BankRtpcRows<'input, R>>,
}

fn observe<'input, R>(
    sites: &NodeSites,
    prepared: &mut BankOrigins<'_, '_, '_>,
    body: &Range<u64>,
    version: u32,
    read: BankRtpcRead<'input, R>,
) -> Result<ObservedRtpc<'input, R>, MediaError> {
    let (rows, error) = match read {
        BankRtpcRead::Readable(rows) => (rows, None),
        BankRtpcRead::Malformed { rows, error, .. } => (rows, Some(error)),
        BankRtpcRead::Absent => {
            return unobserved(sites, prepared, body, RtpcRecord::Absent { version });
        }
        BankRtpcRead::Unavailable(_) => {
            return unobserved(sites, prepared, body, RtpcRecord::Unavailable { version });
        }
        _ => {
            return Err(MediaError::Contract(
                "unexpected node RTPC disposition".into(),
            ));
        }
    };
    let extent = if error.is_some() { body } else { rows.range() };
    sites.summary(prepared, Some(extent), error.is_some())?;
    checked_origins(sites, prepared, &rows)?;
    let curve_count = rows.declared_curve_count().into();
    let checked_curves = rows.checked_curve_count();
    let checked_range = rows.range().into();
    let (record, issue) = if let Some(error) = error {
        let issue = MediaIssue::new(
            "audio.bank.invalid-rtpc",
            "HIRC",
            Severity::Error,
            error.to_string(),
        );
        (
            RtpcRecord::Malformed {
                version,
                curve_count,
                checked_curves,
                checked_range,
            },
            Some(issue),
        )
    } else {
        (
            RtpcRecord::Decoded {
                version,
                curve_count,
                checked_curves,
                checked_range,
            },
            None,
        )
    };
    Ok(ObservedRtpc {
        record,
        issue,
        rows: Some(rows),
    })
}

fn unobserved<'input, R>(
    sites: &NodeSites,
    prepared: &mut BankOrigins<'_, '_, '_>,
    body: &Range<u64>,
    record: RtpcRecord,
) -> Result<ObservedRtpc<'input, R>, MediaError> {
    sites.summary(prepared, Some(body), false)?;
    Ok(ObservedRtpc {
        record,
        issue: None,
        rows: None,
    })
}

fn checked_origins<R>(
    sites: &NodeSites,
    prepared: &mut BankOrigins<'_, '_, '_>,
    rows: &BankRtpcRows<'_, R>,
) -> Result<(), MediaError> {
    sites.field(
        prepared,
        "curve_count",
        &rows.declared_curve_count().range,
        "rtpcCurveCount",
    )?;
    sites.range(prepared, "checked_range", rows.range())?;
    sites.converted(
        prepared,
        "checked_curves",
        rows.range(),
        "rtpcCheckedCurves",
    )
}
