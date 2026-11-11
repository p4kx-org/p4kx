// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};

use p4kx_extraction::bank::{
    BankMusicTrackAutomation, BankMusicTrackAutomationRows, BankTrackAutomationFields,
};

use crate::audio_bank_music_track_automation_points;
use crate::audio_bank_music_track_control_origins;
use crate::audio_bank_music_track_control_payload::{
    AUTOMATION, TrackAutomationRecord, TrackControlPayload,
};
use crate::audio_bank_node_origins::NodeSites;
use crate::audio_bank_origins::BankOrigins;
use crate::audio_bank_projection::BankProjection;
use crate::{MediaError, MediaIssue, MediaWriter, Severity, Subresource};

pub(crate) fn project<R: Read + Seek>(
    projection: &BankProjection<'_>,
    sites: &NodeSites,
    mut rows: BankMusicTrackAutomationRows<'_, R>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    while let Some(mut automation) = rows
        .next_automation()
        .map_err(|cause| projection.replay_failure(cause))?
    {
        emit(projection, sites, &automation, writer)?;
        let ordinal = automation.ordinal;
        let range = automation.fields.clip_index.range.clone();
        if let Some(points) = automation.points() {
            audio_bank_music_track_automation_points::project(
                projection, sites, ordinal, &range, points, writer,
            )?;
        }
    }
    Ok(())
}

fn emit<R>(
    projection: &BankProjection<'_>,
    sites: &NodeSites,
    row: &BankMusicTrackAutomation<'_, R>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let index = &row.fields.clip_index.range;
    let mut prepared = sites.record(projection.input, writer, Some(index))?;
    sites.index(&mut prepared, 2, index, "musicTrackAutomationPosition")?;
    let status = audio_bank_music_track_control_origins::layout(
        sites,
        &mut prepared,
        &row.checked_range,
        row.issue.is_some(),
    )?;
    let record = read(sites, &mut prepared, &row.fields, row.issue.is_some())?;
    let issue = row.issue.as_ref().map(|error| {
        MediaIssue::new(
            "audio.bank.truncated-music-track-automation",
            "HIRC",
            Severity::Error,
            error.to_string(),
        )
    });
    let [section, object] = sites.indices();
    prepared.finish().write(
        writer,
        &Subresource::new(AUTOMATION, vec![section, object, u64::from(row.ordinal)]),
        status,
        &TrackControlPayload::Automation(&record),
        issue.as_slice(),
    )
}

fn read(
    sites: &NodeSites,
    prepared: &mut BankOrigins<'_, '_, '_>,
    original: &BankTrackAutomationFields,
    malformed: bool,
) -> Result<TrackAutomationRecord, MediaError> {
    sites.field(
        prepared,
        "clip_index",
        &original.clip_index.range,
        "musicTrackAutomationClipIndex",
    )?;
    let clip_index = (&original.clip_index).into();
    let automation_type = sites.scalar(
        prepared,
        "automation_type",
        original.automation_type.as_ref(),
        "musicTrackAutomationType",
    )?;
    let points = original
        .points
        .as_ref()
        .map(|points| {
            audio_bank_music_track_control_origins::table(
                sites,
                prepared,
                &points.declared_count,
                &points.extent,
                "points",
                "musicTrackAutomationPointCount",
            )
        })
        .transpose()?;
    match (malformed, automation_type, points) {
        (false, Some(automation_type), Some(points)) => Ok(TrackAutomationRecord::Decoded {
            clip_index,
            automation_type,
            points,
        }),
        (true, automation_type, points) => Ok(TrackAutomationRecord::Malformed {
            clip_index,
            automation_type,
            points,
        }),
        _ => Err(MediaError::Contract(
            "complete MusicTrack automation lacks checked fields".into(),
        )),
    }
}
