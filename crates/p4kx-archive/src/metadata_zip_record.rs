// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::central_directory::CENTRAL_HEADER_LEN;
use crate::entry_record_format::EntryRecordFormat;
use crate::metadata_access::MetadataAccess;
use crate::metadata_read_error::MetadataReadError;
use crate::metadata_record::MetadataRecord;
use crate::metadata_span::MetadataSpan;

pub(crate) fn read<'source>(
    access: &mut MetadataAccess<'source, '_>,
    section: MetadataSpan<'source>,
    relative: u64,
    ordinal: Option<u64>,
    admitted_index: Option<usize>,
) -> Result<MetadataRecord<'source>, MetadataReadError> {
    let mut header = [0_u8; CENTRAL_HEADER_LEN];
    access.read_into(section, relative, &mut header)?;
    let offset = section
        .range()
        .offset()
        .checked_add(relative)
        .ok_or(MetadataReadError::OutsideSpan)?;
    let lengths = crate::central_directory::record_lengths(&header, offset).map_err(|source| {
        MetadataReadError::Parse {
            offset,
            source: Box::new(source),
        }
    })?;
    let name_len = u64::try_from(lengths.name).map_err(|_| MetadataReadError::OutsideSpan)?;
    let extra_len = u64::try_from(lengths.extra).map_err(|_| MetadataReadError::OutsideSpan)?;
    let comment_len = u64::try_from(lengths.comment).map_err(|_| MetadataReadError::OutsideSpan)?;
    let extra_start = (CENTRAL_HEADER_LEN as u64)
        .checked_add(name_len)
        .ok_or(MetadataReadError::OutsideSpan)?;
    let comment_start = extra_start
        .checked_add(extra_len)
        .ok_or(MetadataReadError::OutsideSpan)?;
    let total = comment_start
        .checked_add(comment_len)
        .ok_or(MetadataReadError::OutsideSpan)?;
    let raw = section.subspan(relative, total)?;
    Ok(MetadataRecord {
        raw,
        format: EntryRecordFormat::ZipCentralDirectory,
        ordinal,
        admitted_index,
        name: Some(raw.subspan(CENTRAL_HEADER_LEN as u64, name_len)?),
        extra: Some(raw.subspan(extra_start, extra_len)?),
        comment: Some(raw.subspan(comment_start, comment_len)?),
    })
}
