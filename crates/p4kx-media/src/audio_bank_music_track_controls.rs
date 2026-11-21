// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};

use p4kx_extraction::bank::{
    BankHircMusicTrack, BankHircObject, BankMusicTrackFields, BankMusicTrackRead,
};

use crate::audio_bank_inventory::VersionSite;
use crate::audio_bank_music_track_automation;
use crate::audio_bank_music_track_control_origins;
use crate::audio_bank_music_track_control_payload::{
    CONTROLS, TrackControlObservations, TrackControlPayload,
};
use crate::audio_bank_music_track_transition;
use crate::audio_bank_node_declarations::{Coverage, Declarations, Disposition};
use crate::audio_bank_node_origins::{NodeKind, NodeSites};
use crate::audio_bank_origins::BankOrigins;
use crate::audio_bank_projection::BankProjection;
use crate::{MediaError, MediaWriter, Subresource};

pub(crate) fn project<R: Read + Seek>(
    projection: &BankProjection<'_>,
    version: VersionSite,
    track: &mut BankHircMusicTrack<'_, R>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let declarations = Declarations::new(projection, &track.object, version);
    let disposition = match &track.status {
        BankMusicTrackRead::Decoded => Disposition::Observed(None),
        BankMusicTrackRead::Malformed(error) => Disposition::Observed(Some(error)),
        BankMusicTrackRead::Unsupported(_) => Disposition::Unsupported,
        _ => {
            return Err(MediaError::Contract(
                "unexpected MusicTrack controls disposition".into(),
            ));
        }
    };
    let sites = declarations.sites(NodeKind::MusicTrackControls);
    summary(&declarations, &sites, disposition, &track.fields, writer)?;
    if let Some(rows) = track.automations() {
        audio_bank_music_track_automation::project(projection, &sites, rows, writer)?;
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
    let sites = declarations.sites(NodeKind::MusicTrackControls);
    summary(
        &declarations,
        &sites,
        Disposition::Unsupported,
        &BankMusicTrackFields::default(),
        writer,
    )
}

fn summary(
    declarations: &Declarations<'_, '_>,
    sites: &NodeSites,
    disposition: Disposition<'_>,
    original: &BankMusicTrackFields,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let mut prepared = sites.record(declarations.bank.input, writer, None)?;
    let observed = observations(sites, &mut prepared, original)?;
    let complete_range = original
        .controls_start
        .zip(original.look_ahead_bits.as_ref())
        .map(|(start, look_ahead)| start..look_ahead.range.end);
    let coverage = Coverage::read(original.controls_start, complete_range);
    let (record, status, issue) = declarations.summary(
        sites,
        &mut prepared,
        disposition,
        &coverage,
        observed,
        "audio.bank.invalid-music-track-controls",
    )?;
    prepared.finish().write(
        writer,
        &Subresource::new(CONTROLS, sites.indices().to_vec()),
        status,
        &TrackControlPayload::Controls(&record),
        issue.as_slice(),
    )
}

fn observations(
    sites: &NodeSites,
    prepared: &mut BankOrigins<'_, '_, '_>,
    original: &BankMusicTrackFields,
) -> Result<TrackControlObservations, MediaError> {
    let automation = original
        .automation_count
        .as_ref()
        .zip(original.automation_extent.as_ref())
        .map(|(count, extent)| {
            audio_bank_music_track_control_origins::table(
                sites,
                prepared,
                count,
                extent,
                "observed/automation",
                "musicTrackAutomationCount",
            )
        })
        .transpose()?;
    let transition = original
        .switch
        .as_ref()
        .and_then(|switch| switch.transition.as_ref())
        .map(|fields| audio_bank_music_track_transition::observe(sites, prepared, fields))
        .transpose()?;
    Ok(TrackControlObservations {
        flags: sites.scalar(
            prepared,
            "observed/flags",
            original.flags.as_ref(),
            "musicTrackFlags",
        )?,
        subtrack_count: sites.scalar(
            prepared,
            "observed/subtrack_count",
            original.subtrack_count.as_ref(),
            "musicTrackSubtrackCount",
        )?,
        look_ahead: audio_bank_music_track_control_origins::signed(
            sites,
            prepared,
            original.look_ahead_bits.as_ref(),
            "observed/look_ahead",
            "musicTrackLookAheadBits",
        )?,
        automation,
        transition,
    })
}
