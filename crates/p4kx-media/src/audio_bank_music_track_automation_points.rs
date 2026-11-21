// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};
use std::ops::Range;

use p4kx_extraction::bank::{BankGraphPoint, BankGraphPointRows, BankHircField};

use crate::audio_bank_music_track_control_origins;
use crate::audio_bank_music_track_control_payload::{
    POINT, TrackAutomationPoint, TrackAutomationWord, TrackControlPayload,
};
use crate::audio_bank_node_origins::NodeSites;
use crate::audio_bank_origins::BankOrigins;
use crate::audio_bank_projection::BankProjection;
use crate::{MediaError, MediaIssue, MediaWriter, Severity, Subresource};

pub(crate) fn project<R: Read + Seek>(
    projection: &BankProjection<'_>,
    sites: &NodeSites,
    automation: u32,
    index: &Range<u64>,
    rows: BankGraphPointRows<'_, R>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    for row in rows {
        let row = row.map_err(|cause| projection.replay_failure(cause))?;
        emit(projection, sites, automation, index, &row, writer)?;
    }
    Ok(())
}

fn emit(
    projection: &BankProjection<'_>,
    sites: &NodeSites,
    automation: u32,
    index: &Range<u64>,
    row: &BankGraphPoint,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let range = &row.from_bits.range;
    let mut prepared = sites.record(projection.input, writer, Some(range))?;
    sites.index(&mut prepared, 2, index, "musicTrackAutomationPosition")?;
    sites.index(&mut prepared, 3, range, "musicTrackAutomationPointPosition")?;
    let record = read(sites, &mut prepared, row)?;
    let malformed = matches!(record, TrackAutomationPoint::Malformed { .. });
    let status = audio_bank_music_track_control_origins::layout(
        sites,
        &mut prepared,
        &row.checked_range,
        malformed,
    )?;
    let issue = malformed.then(|| {
        MediaIssue::new(
            "audio.bank.truncated-music-track-automation-point",
            "HIRC",
            Severity::Error,
            "the original automation point ends before all its words fit",
        )
    });
    let [section, object] = sites.indices();
    prepared.finish().write(
        writer,
        &Subresource::new(
            POINT,
            vec![
                section,
                object,
                u64::from(automation),
                u64::from(row.ordinal),
            ],
        ),
        status,
        &TrackControlPayload::Point(&record),
        issue.as_slice(),
    )
}

fn read(
    sites: &NodeSites,
    prepared: &mut BankOrigins<'_, '_, '_>,
    row: &BankGraphPoint,
) -> Result<TrackAutomationPoint, MediaError> {
    let from = word(
        sites,
        prepared,
        &row.from_bits,
        "from",
        "musicTrackAutomationFromBits",
    )?;
    let to = row
        .to_bits
        .as_ref()
        .map(|word_bits| {
            word(
                sites,
                prepared,
                word_bits,
                "to",
                "musicTrackAutomationToBits",
            )
        })
        .transpose()?;
    let interpolation = sites.scalar(
        prepared,
        "interpolation",
        row.interpolation.as_ref(),
        "musicTrackAutomationInterpolation",
    )?;
    match (to, interpolation) {
        (Some(to), Some(interpolation)) => Ok(TrackAutomationPoint::Decoded {
            from,
            to,
            interpolation,
        }),
        (to, None) => Ok(TrackAutomationPoint::Malformed { from, to }),
        _ => Err(MediaError::Contract(
            "automation interpolation lacks its preceding word".into(),
        )),
    }
}

fn word(
    sites: &NodeSites,
    prepared: &mut BankOrigins<'_, '_, '_>,
    original: &BankHircField<u32>,
    name: &str,
    label: &'static str,
) -> Result<TrackAutomationWord, MediaError> {
    sites.field(prepared, &format!("{name}/bits"), &original.range, label)?;
    let value = f32::from_bits(original.value);
    let finite_value = value.is_finite().then_some(value);
    if finite_value.is_some() {
        sites.converted(
            prepared,
            &format!("{name}/finite_value"),
            &original.range,
            "musicTrackAutomationFiniteValue",
        )?;
    }
    Ok(TrackAutomationWord {
        bits: original.into(),
        finite_value,
    })
}
