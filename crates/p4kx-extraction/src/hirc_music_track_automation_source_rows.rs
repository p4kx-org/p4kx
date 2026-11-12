// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};

use p4kx_audio::{
    BankGraphPoint, BankHircField, BankMusicTrackAutomation, BankMusicTrackAutomationRows,
};
use serde::ser::{Error as _, SerializeSeq};

use crate::source_evidence_error::SourceEvidenceError;

use super::super::dto::MusicTrackSourceRow;
use super::super::summary;
use super::ObjectOrigins;

pub(super) fn serialize<R: Read + Seek, S: SerializeSeq>(
    sequence: &mut S,
    origins: &ObjectOrigins,
    mut rows: BankMusicTrackAutomationRows<'_, R>,
) -> Result<(), S::Error> {
    while let Some(mut original) = rows.next_automation().map_err(S::Error::custom)? {
        let row = observation(origins, &original).map_err(S::Error::custom)?;
        sequence.serialize_element(&row)?;
        let ordinal = original.ordinal;
        let clip_index = original.fields.clip_index.clone();
        if let Some(points) = original.points() {
            for original in points {
                let original = original.map_err(S::Error::custom)?;
                let point =
                    point(origins, ordinal, &clip_index, &original).map_err(S::Error::custom)?;
                sequence.serialize_element(&point)?;
            }
        }
    }
    Ok(())
}

fn observation<R>(
    origins: &ObjectOrigins,
    original: &BankMusicTrackAutomation<'_, R>,
) -> Result<MusicTrackSourceRow, SourceEvidenceError> {
    let fields = &original.fields;
    let points = fields
        .points
        .as_ref()
        .map(|points| {
            summary::table(
                origins,
                &points.declared_count,
                &points.extent,
                "musicTrackAutomationPointCount",
                "musicTrackAutomationPointsExtent",
            )
        })
        .transpose()?;
    Ok(MusicTrackSourceRow::Automation {
        ordinal: origins.derived(
            original.ordinal,
            &fields.clip_index.range,
            "musicTrackAutomationPosition",
        )?,
        range: origins.range(&original.checked_range, "musicTrackAutomationRowExtent")?,
        clip_index: origins.field(&fields.clip_index, "musicTrackAutomationClipIndex")?,
        automation_type: origins
            .optional(fields.automation_type.as_ref(), "musicTrackAutomationType")?,
        points,
    })
}

fn point(
    origins: &ObjectOrigins,
    automation: u32,
    clip_index: &BankHircField<u32>,
    original: &BankGraphPoint,
) -> Result<MusicTrackSourceRow, SourceEvidenceError> {
    Ok(MusicTrackSourceRow::AutomationPoint {
        automation_ordinal: origins.derived(
            automation,
            &clip_index.range,
            "musicTrackAutomationPosition",
        )?,
        point_ordinal: origins.derived(
            original.ordinal,
            &original.from_bits.range,
            "musicTrackAutomationPointPosition",
        )?,
        range: origins.range(&original.checked_range, "musicTrackAutomationPointExtent")?,
        from_bits: origins.field(&original.from_bits, "musicTrackAutomationFromBits")?,
        to_bits: origins.optional(original.to_bits.as_ref(), "musicTrackAutomationToBits")?,
        interpolation: origins.optional(
            original.interpolation.as_ref(),
            "musicTrackAutomationInterpolation",
        )?,
    })
}
