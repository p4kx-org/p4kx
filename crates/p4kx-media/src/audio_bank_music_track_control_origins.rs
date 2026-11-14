// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_extraction::bank::{BankHircField, BankTrackAutomationExtent};

use crate::audio_bank_music_track_control_payload::{
    TrackAutomationCoverage, TrackAutomationTable, TrackSignedWord,
};
use crate::audio_bank_node_origins::NodeSites;
use crate::audio_bank_origins::BankOrigins;
use crate::{MediaError, Status};

pub(crate) fn table(
    sites: &NodeSites,
    prepared: &mut BankOrigins<'_, '_, '_>,
    count: &BankHircField<u32>,
    extent: &BankTrackAutomationExtent,
    name: &str,
    label: &'static str,
) -> Result<TrackAutomationTable, MediaError> {
    let range = &extent.checked_range;
    sites.field(
        prepared,
        &format!("{name}/declared_count"),
        &count.range,
        label,
    )?;
    sites.range(prepared, &format!("{name}/checked_range"), range)?;
    let coverage = if extent.is_complete() {
        TrackAutomationCoverage::Complete
    } else {
        TrackAutomationCoverage::Truncated
    };
    sites.converted(
        prepared,
        &format!("{name}/coverage"),
        &(count.range.start..range.end),
        "musicTrackAutomationCoverage",
    )?;
    Ok(TrackAutomationTable {
        declared_count: count.into(),
        checked_range: range.into(),
        coverage,
    })
}

pub(crate) fn signed(
    sites: &NodeSites,
    prepared: &mut BankOrigins<'_, '_, '_>,
    original: Option<&BankHircField<u32>>,
    name: &str,
    label: &'static str,
) -> Result<Option<TrackSignedWord>, MediaError> {
    let Some(word) = original else {
        return Ok(None);
    };
    sites.field(prepared, &format!("{name}/bits"), &word.range, label)?;
    sites.converted(
        prepared,
        &format!("{name}/signed_value"),
        &word.range,
        "musicTrackSignedWord",
    )?;
    Ok(Some(TrackSignedWord {
        bits: word.into(),
        signed_value: i32::from_le_bytes(word.value.to_le_bytes()),
    }))
}

pub(crate) fn layout(
    sites: &NodeSites,
    prepared: &mut BankOrigins<'_, '_, '_>,
    range: &Range<u64>,
    malformed: bool,
) -> Result<Status, MediaError> {
    sites.converted(prepared, "layout", range, "musicTrackAutomationRowCoverage")?;
    if malformed {
        sites.issues(prepared, range)?;
        Ok(Status::Partial)
    } else {
        Ok(Status::Complete)
    }
}
