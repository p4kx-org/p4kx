// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use crate::bank_reader::SectionReader;
use crate::bank_stid;
use crate::bank_structure::{self, DIDX_ENTRY_LEN, MediaTriplet, SECTION_HEADER_LEN, SeenSections};
use crate::soundbank::BankHeader;
use crate::{BankError, BankWarning, HircObject, SectionId, hirc};

const LAST_UNSUPPORTED_VERSION: u32 = 48;

#[derive(Debug, Clone)]
pub(crate) struct SectionRange {
    pub(crate) id: SectionId,
    pub(crate) body: Range<usize>,
}

#[derive(Debug, Clone)]
pub(crate) struct MediaRange {
    pub(crate) id: u32,
    pub(crate) offset: u64,
    pub(crate) range: Range<usize>,
}

pub(crate) struct Parsed {
    pub(crate) header: BankHeader,
    pub(crate) sections: Vec<SectionRange>,
    pub(crate) media: Vec<MediaRange>,
    pub(crate) objects: Vec<HircObject>,
    pub(crate) names: Vec<(u32, Range<usize>)>,
    pub(crate) warnings: Vec<BankWarning>,
}

pub(crate) fn parse(bytes: &[u8]) -> Result<Parsed, BankError> {
    if bytes.first_chunk::<4>() != Some(&SectionId::BKHD.0) {
        return Err(BankError::NotSoundbank);
    }
    let sections = sections(bytes)?;
    let body = |id: SectionId| {
        sections
            .iter()
            .find(|section| section.id == id)
            .map(|section| {
                (
                    section.body.clone(),
                    bytes.get(section.body.clone()).unwrap_or_default(),
                )
            })
    };
    let (_, header) = body(SectionId::BKHD).ok_or(BankError::NotSoundbank)?;
    let header = parse_header(header)?;
    let mut warnings = Vec::new();
    let media = match (body(SectionId::DIDX), body(SectionId::DATA)) {
        (Some((_, index)), Some((data, _))) => media(index, data, &mut warnings)?,
        (Some((_, index)), None) => {
            warnings.push(BankWarning::MediaWithoutData {
                entries: (index.len() / DIDX_ENTRY_LEN) as u64,
            });
            Vec::new()
        }
        (None, _) => Vec::new(),
    };
    let objects = match body(SectionId::HIRC) {
        Some((range, hirc)) => {
            let hirc = hirc::parse(header.version, hirc, range.start)?;
            if hirc.trailing > 0 {
                warnings.push(BankWarning::TrailingHircBytes {
                    len: hirc.trailing as u64,
                });
            }
            hirc.objects
        }
        None => Vec::new(),
    };
    let names = match body(SectionId::STID) {
        Some((range, stid)) => bank_stid::parse_names(stid, range.start, &mut warnings)?,
        None => Vec::new(),
    };
    Ok(Parsed {
        header,
        sections,
        media,
        objects,
        names,
        warnings,
    })
}

fn sections(bytes: &[u8]) -> Result<Vec<SectionRange>, BankError> {
    let mut sections = Vec::new();
    let mut seen = SeenSections::default();
    let mut position = 0;
    while position < bytes.len() {
        let rest = bytes.get(position..).unwrap_or_default();
        let layout = bank_structure::section_layout(rest, rest.len() as u64)?;
        if seen.repeat(layout.id) {
            return Err(BankError::DuplicateSection(layout.id));
        }
        let end = usize::try_from(SECTION_HEADER_LEN as u64 + layout.body_len).map_err(|_| {
            BankError::truncated(
                layout.id,
                rest.len(),
                SECTION_HEADER_LEN as u64 + layout.body_len,
            )
        })?;
        sections.push(SectionRange {
            id: layout.id,
            body: position + SECTION_HEADER_LEN..position + end,
        });
        position += end;
    }
    Ok(sections)
}

pub(crate) fn parse_header(body: &[u8]) -> Result<BankHeader, BankError> {
    header_with(body, |version| {
        if supported_version(version) {
            Ok(())
        } else {
            Err(BankError::UnsupportedVersion { version })
        }
    })
}

pub(crate) fn raw_header(body: &[u8]) -> Result<BankHeader, BankError> {
    header_with(body, |_| Ok(()))
}

pub(crate) const fn supported_version(version: u32) -> bool {
    version > LAST_UNSUPPORTED_VERSION && version <= u16::MAX as u32
}

fn header_with(
    body: &[u8],
    validate_version: impl FnOnce(u32) -> Result<(), BankError>,
) -> Result<BankHeader, BankError> {
    let mut reader = SectionReader::new(SectionId::BKHD, body);
    let version = reader.u32()?;
    validate_version(version)?;
    Ok(BankHeader {
        version,
        id: reader.u32()?,
        language_id: reader.u32()?,
    })
}

fn media(
    index: &[u8],
    data: Range<usize>,
    warnings: &mut Vec<BankWarning>,
) -> Result<Vec<MediaRange>, BankError> {
    if !index.len().is_multiple_of(DIDX_ENTRY_LEN) {
        return Err(BankError::invalid(
            SectionId::DIDX,
            "its size is not a multiple of 12 bytes",
        ));
    }
    let data_len = data.len() as u64;
    let mut reader = SectionReader::new(SectionId::DIDX, index);
    let mut media = Vec::with_capacity(index.len() / DIDX_ENTRY_LEN);
    while reader.remaining() > 0 {
        let triplet = MediaTriplet::read(&mut reader)?;
        let range = triplet.range_in(data.start as u64, data_len)?;
        let start = usize::try_from(range.start).map_err(|_| triplet.outside_data(data_len))?;
        let end = usize::try_from(range.end).map_err(|_| triplet.outside_data(data_len))?;
        media.push(MediaRange {
            id: triplet.id,
            offset: u64::from(triplet.offset),
            range: start..end,
        });
    }
    media.sort_by_key(|media| media.id);
    media.dedup_by(|later, first| {
        let repeated = later.id == first.id;
        if repeated {
            warnings.push(BankWarning::DuplicateMedia { id: later.id });
        }
        repeated
    });
    Ok(media)
}
