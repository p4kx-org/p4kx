// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};

use p4kx_extraction::bank::{
    BankDidx, BankDidxEntry, BankMediaInfo, BankReadError, BankSource, BankSourceSection,
};
use p4kx_extraction::{BankError, DecimalU64, MediaClass};

use crate::audio_bank_failure::BankPhase;
use crate::audio_bank_inventory::DataInventory;
use crate::audio_bank_payload::{BankDataLink, BankEntryRange, BankMediaRecord, BankPayload};
use crate::audio_bank_projection::BankProjection;
use crate::audio_wem_projection::{self, WemProjection};
use crate::{
    MediaError, MediaIssue, MediaWriter, Severity, Status, Subresource, audio_bank_media_origins,
};

pub(crate) const MEDIA_NAMESPACE: &str = "audio-bank-media";
const DIDX_WORDS_BYTES: u64 = 3 * size_of::<u32>() as u64;

pub(crate) enum DataResolution<'data> {
    Resolved {
        section: &'data BankSourceSection,
        info: BankMediaInfo,
    },
    Missing,
    Ambiguous(u64),
    Unavailable,
    OutsideData {
        section: &'data BankSourceSection,
    },
}

pub(crate) fn project<R: Read + Seek>(
    projection: &BankProjection<'_>,
    source: &mut BankSource<R>,
    section: &BankSourceSection,
    data: Option<&DataInventory>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let mut index = match source.didx(section) {
        Ok(index) => index,
        Err(cause) => {
            return projection.failure(BankPhase::MediaIndex, Some(section), cause, writer);
        }
    };
    while let Some(result) = index.next() {
        let entry = match result {
            Ok(entry) => entry,
            Err(cause) => {
                return projection.failure(BankPhase::MediaIndex, Some(section), cause, writer);
            }
        };
        let resolution =
            resolve(&mut index, &entry, data).map_err(|cause| projection.replay_failure(cause))?;
        emit(projection, &entry, &resolution, writer)?;
        project_embedded(projection, &mut index, &entry, &resolution, writer)?;
    }
    Ok(())
}

fn project_embedded<R: Read + Seek>(
    projection: &BankProjection<'_>,
    index: &mut BankDidx<'_, R>,
    entry: &BankDidxEntry,
    resolution: &DataResolution<'_>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let DataResolution::Resolved { section, info } = resolution else {
        return Ok(());
    };
    if info.class != MediaClass::Wem || info.prefetch {
        return Ok(());
    }
    let reader = index
        .media_reader(entry, section)
        .map_err(|cause| projection.replay_failure(cause))?;
    let origins =
        audio_bank_media_origins::embedded_wem(projection.input, entry, section, info, writer)?;
    let wem = WemProjection::embedded(
        projection.input,
        projection.path,
        [entry.section_ordinal(), entry.ordinal()],
        &info.range,
        origins,
    )?;
    audio_wem_projection::project(&wem, reader, writer)
}

fn resolve<'data, R: Read + Seek>(
    index: &mut BankDidx<'_, R>,
    entry: &BankDidxEntry,
    data: Option<&'data DataInventory>,
) -> Result<DataResolution<'data>, BankReadError> {
    Ok(match data {
        None => DataResolution::Unavailable,
        Some(DataInventory::Missing) => DataResolution::Missing,
        Some(DataInventory::Ambiguous(candidates)) => DataResolution::Ambiguous(*candidates),
        Some(DataInventory::Unique(section)) => match index.inspect_media(entry, section) {
            Ok(info) => DataResolution::Resolved { section, info },
            Err(BankReadError::Format(BankError::MediaOutsideData { .. })) => {
                DataResolution::OutsideData { section }
            }
            Err(cause) => return Err(cause),
        },
    })
}

fn emit(
    projection: &BankProjection<'_>,
    entry: &BankDidxEntry,
    resolution: &DataResolution<'_>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let index_start = entry.triplet_offset();
    let index_end = index_start
        .checked_add(DIDX_WORDS_BYTES)
        .ok_or_else(crate::origin_error::counter)?;
    let payload = BankPayload::BankMedia(BankMediaRecord {
        section_ordinal: DecimalU64::new(entry.section_ordinal()),
        ordinal: DecimalU64::new(entry.ordinal()),
        media_id: entry.media_id(),
        data_offset: entry.offset(),
        data_length: entry.length(),
        index_range: BankEntryRange::new(index_start, index_end),
        data: resolution.payload(),
    });
    let (status, issue) = resolution.status();
    audio_bank_media_origins::prepare(projection.input, entry, resolution, writer)?.write(
        writer,
        &Subresource::new(
            MEDIA_NAMESPACE,
            vec![entry.section_ordinal(), entry.ordinal()],
        ),
        status,
        &payload,
        issue.as_slice(),
    )
}

impl DataResolution<'_> {
    fn payload(&self) -> BankDataLink {
        match self {
            Self::Resolved { section, info } => BankDataLink::Resolved {
                section_ordinal: DecimalU64::new(section.ordinal()),
                range: BankEntryRange::new(info.range.start, info.range.end),
                class: info.class.name(),
                declared_wem_length: info.declared_len.map(DecimalU64::new),
                prefetch: info.prefetch,
            },
            Self::Missing => BankDataLink::Missing,
            Self::Ambiguous(candidates) => BankDataLink::Ambiguous {
                candidates: DecimalU64::new(*candidates),
            },
            Self::Unavailable => BankDataLink::Unavailable,
            Self::OutsideData { section } => BankDataLink::OutsideData {
                section_ordinal: DecimalU64::new(section.ordinal()),
                data_length: DecimalU64::new(section.body_len()),
            },
        }
    }

    fn status(&self) -> (Status, Option<MediaIssue>) {
        match self {
            Self::Missing => (
                Status::Complete,
                Some(MediaIssue::new(
                    "audio.bank.media-without-data",
                    "DIDX",
                    Severity::Info,
                    "The soundbank contains no DATA section for this DIDX declaration",
                )),
            ),
            Self::OutsideData { .. } => (
                Status::Partial,
                Some(MediaIssue::new(
                    "audio.bank.media-outside-data",
                    "DIDX",
                    Severity::Warning,
                    "The original DIDX range extends beyond the selected DATA body",
                )),
            ),
            _ => (Status::Complete, None),
        }
    }
}
