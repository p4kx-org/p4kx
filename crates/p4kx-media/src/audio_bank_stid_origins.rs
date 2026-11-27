// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourceUseRole;
use p4kx_extraction::bank::{BankSourceSection, BankStidName};

use crate::audio_bank_origins::{BankOrigins, EvidenceSite, section_routing};
use crate::origin_capture::OriginSite;
use crate::origin_input::MediaOriginInput;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::{MediaError, MediaWriter};

pub(crate) fn header(
    input: &MediaOriginInput,
    section: &BankSourceSection,
    writer: &MediaWriter<'_>,
) -> Result<PreparedRecordOrigins, MediaError> {
    let mut prepared = BankOrigins::new(input, writer)?;
    let ordinal = [section.ordinal()];
    let body = [section.body_offset()];
    let route = section_routing(&ordinal);
    section_index(&mut prepared, route)?;
    for (path, field) in [
        ("/data/payload/string_type", "stringType"),
        ("/data/payload/declared_names", "nameCount"),
    ] {
        let evidence = prepared.capture([
            route,
            (
                OriginSite::new("bnkStid", &body, field),
                SourceUseRole::Payload,
            ),
        ])?;
        prepared.field(path, evidence)?;
    }
    Ok(prepared.finish())
}

pub(crate) fn name(
    input: &MediaOriginInput,
    name: &BankStidName,
    invalid_utf8: bool,
    writer: &MediaWriter<'_>,
) -> Result<PreparedRecordOrigins, MediaError> {
    let mut prepared = BankOrigins::new(input, writer)?;
    let ordinal = [name.section_ordinal(), name.ordinal()];
    let header = [name.header_offset()];
    let length = (
        OriginSite::new("bnkStidName", &header, "nameLength"),
        SourceUseRole::Payload,
    );
    let route = (
        OriginSite::new("bnkStidPrefix", &ordinal, "nameRouting"),
        SourceUseRole::Selector,
    );
    let section = prepared.capture([section_routing(&[name.section_ordinal()])])?;
    let position = prepared.capture([route])?;
    let extent = prepared.capture([route, length])?;
    prepared.field("/data/payload/section_ordinal", section.clone())?;
    prepared.field("/subresource/indices/0", section)?;
    prepared.field("/data/payload/ordinal", position.clone())?;
    prepared.indices(position.clone())?;
    prepared.field("/data/payload/name_length", extent.clone())?;
    let id = prepared.capture([
        route,
        (
            OriginSite::new("bnkStidName", &header, "bankId"),
            SourceUseRole::Payload,
        ),
    ])?;
    prepared.field("/data/payload/bank_id", id)?;
    prepared.range("/data/payload/header_range", &position, &position)?;
    prepared.range("/data/payload/name_range", &position, &extent)?;
    name_bytes(&mut prepared, name, route, length, invalid_utf8)?;
    Ok(prepared.finish())
}

fn name_bytes(
    prepared: &mut BankOrigins<'_, '_, '_>,
    name: &BankStidName,
    route: EvidenceSite<'_>,
    length: EvidenceSite<'_>,
    invalid_utf8: bool,
) -> Result<(), MediaError> {
    let start = name.name_range().start;
    let span = [start];
    let mut bytes = vec![route, length];
    let mut text = bytes.clone();
    if name.name_len() > 0 {
        let site = OriginSite::new("bnkStidName", &span, "nameBytes");
        bytes.push((site, SourceUseRole::Payload));
        text.push((site, SourceUseRole::Conversion));
    }
    let original = prepared.capture(bytes)?;
    prepared.field("/data/payload/name_bytes", original)?;
    for position in 0..u64::from(name.name_len()) {
        let absolute = [start + position];
        let byte = prepared.capture([
            route,
            (
                OriginSite::new("bnkByte", &absolute, "stidNameByte"),
                SourceUseRole::Payload,
            ),
        ])?;
        prepared.field(&format!("/data/payload/name_bytes/{position}"), byte)?;
    }
    let interpreted = prepared.capture(text)?;
    prepared.subtree("/data/payload/name_text", interpreted.clone())?;
    if invalid_utf8 {
        prepared.subtree("/issues", interpreted)?;
    }
    Ok(())
}

pub(crate) fn tail(
    input: &MediaOriginInput,
    section: &BankSourceSection,
    writer: &MediaWriter<'_>,
) -> Result<PreparedRecordOrigins, MediaError> {
    let mut prepared = BankOrigins::new(input, writer)?;
    let ordinal = [section.ordinal()];
    let body = [section.body_offset()];
    let route = section_routing(&ordinal);
    section_index(&mut prepared, route)?;
    let prefix = (
        OriginSite::new("bnkStidPrefix", &ordinal, "completeNameRouting"),
        SourceUseRole::Conversion,
    );
    let start = prepared.capture([route, prefix])?;
    let end = prepared.capture([
        route,
        prefix,
        (
            OriginSite::new("bnkSection", &body, "bodyLength"),
            SourceUseRole::Conversion,
        ),
    ])?;
    prepared.range("/data/payload/trailing_range", &start, &end)?;
    Ok(prepared.finish())
}

fn section_index(
    prepared: &mut BankOrigins<'_, '_, '_>,
    route: EvidenceSite<'_>,
) -> Result<(), MediaError> {
    let section = prepared.capture([route])?;
    prepared.field("/data/payload/section_ordinal", section.clone())?;
    prepared.indices(section)
}
