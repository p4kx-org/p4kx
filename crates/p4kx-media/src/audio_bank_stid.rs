// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};
use std::num::TryFromIntError;
use std::ops::Range;

use p4kx_extraction::DecimalU64;
use p4kx_extraction::bank::{BankSource, BankSourceSection, BankStidName};

use crate::audio_bank_failure::BankPhase;
use crate::audio_bank_payload::BankEntryRange;
use crate::audio_bank_projection::BankProjection;
use crate::audio_bank_stid_payload::{
    BankNameRecord, BankNameText, BankStidHeader, BankStidPayload, BankStidTail,
};
use crate::{
    MediaError, MediaIssue, MediaWriter, Severity, Status, Subresource, audio_bank_stid_origins,
};

pub(crate) const STID_NAMESPACE: &str = "audio-bank-stid";
pub(crate) const NAME_NAMESPACE: &str = "audio-bank-name";
pub(crate) const STID_TAIL_NAMESPACE: &str = "audio-bank-stid-tail";

pub(crate) fn project<R: Read + Seek>(
    projection: &BankProjection<'_>,
    source: &mut BankSource<R>,
    section: &BankSourceSection,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let mut names = match source.stid(section) {
        Ok(names) => names,
        Err(cause) => return projection.failure(BankPhase::Names, Some(section), cause, writer),
    };
    header(
        projection,
        section,
        names.string_type(),
        names.name_count(),
        writer,
    )?;
    for result in names.by_ref() {
        match result {
            Ok(name) => emit_name(projection, &name, writer)?,
            Err(cause) => {
                return projection.failure(BankPhase::Names, Some(section), cause, writer);
            }
        }
    }
    let trailing = names.trailing_range().ok_or_else(|| {
        MediaError::Contract("completed STID cursor has no trailing range".into())
    })?;
    tail(projection, section, &trailing, writer)
}

fn header(
    projection: &BankProjection<'_>,
    section: &BankSourceSection,
    string_type: u32,
    declared_names: u32,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let payload = BankStidPayload::Stid(BankStidHeader {
        section_ordinal: DecimalU64::new(section.ordinal()),
        string_type,
        declared_names,
    });
    audio_bank_stid_origins::header(projection.input, section, writer)?.write(
        writer,
        &Subresource::new(STID_NAMESPACE, vec![section.ordinal()]),
        Status::Complete,
        &payload,
        &[],
    )
}

fn emit_name(
    projection: &BankProjection<'_>,
    name: &BankStidName,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let (name_text, issue) = text(name)?;
    let range = name.name_range();
    let payload = BankStidPayload::Name(BankNameRecord {
        section_ordinal: DecimalU64::new(name.section_ordinal()),
        ordinal: DecimalU64::new(name.ordinal()),
        bank_id: name.id(),
        name_length: name.name_len(),
        header_range: BankEntryRange::new(name.header_offset(), range.start),
        name_range: BankEntryRange::new(range.start, range.end),
        name_bytes: name.bytes(),
        name_text,
    });
    audio_bank_stid_origins::name(projection.input, name, issue.is_some(), writer)?.write(
        writer,
        &Subresource::new(NAME_NAMESPACE, vec![name.section_ordinal(), name.ordinal()]),
        Status::Complete,
        &payload,
        issue.as_slice(),
    )
}

fn text(name: &BankStidName) -> Result<(BankNameText<'_>, Option<MediaIssue>), MediaError> {
    let cause = match name.utf8() {
        Ok(text) => return Ok((BankNameText::Utf8 { text }, None)),
        Err(cause) => cause,
    };
    let offset = u8::try_from(cause.valid_up_to()).map_err(invalid_utf8_offset)?;
    let length = cause
        .error_len()
        .map(u8::try_from)
        .transpose()
        .map_err(invalid_utf8_offset)?;
    Ok((
        BankNameText::InvalidUtf8 {
            valid_up_to: offset,
            error_length: length,
        },
        Some(MediaIssue::new(
            "audio.bank.non-utf8-name",
            "STID",
            Severity::Info,
            format!(
                "Bank name {} contains invalid UTF-8 at byte {offset}",
                name.id()
            ),
        )),
    ))
}

fn invalid_utf8_offset(_cause: TryFromIntError) -> MediaError {
    MediaError::Contract("STID UTF-8 result exceeds the original name width".into())
}

fn tail(
    projection: &BankProjection<'_>,
    section: &BankSourceSection,
    trailing: &Range<u64>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let payload = BankStidPayload::Tail(BankStidTail {
        section_ordinal: DecimalU64::new(section.ordinal()),
        trailing_range: BankEntryRange::new(trailing.start, trailing.end),
    });
    audio_bank_stid_origins::tail(projection.input, section, writer)?.write(
        writer,
        &Subresource::new(STID_TAIL_NAMESPACE, vec![section.ordinal()]),
        Status::Complete,
        &payload,
        &[],
    )
}
