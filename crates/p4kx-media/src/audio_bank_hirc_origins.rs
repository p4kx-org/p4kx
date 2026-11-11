// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourceUseRole;
use p4kx_extraction::bank::{BankHircObject, BankSourceSection};

use crate::audio_bank_inventory::VersionSite;
use crate::audio_bank_origins::{BankOrigins, EvidenceSite, section_routing};
use crate::origin_capture::OriginSite;
use crate::origin_contributions::OriginContributions;
use crate::origin_input::MediaOriginInput;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::{MediaError, MediaWriter};

pub(crate) fn object(
    input: &MediaOriginInput,
    object: &BankHircObject,
    version: Option<VersionSite>,
    writer: &MediaWriter<'_>,
) -> Result<PreparedRecordOrigins, MediaError> {
    let mut prepared = BankOrigins::new(input, writer)?;
    let section = [object.section_ordinal()];
    let ordinal = [object.section_ordinal(), object.ordinal()];
    let body = [object.body_offset()];
    let route = (
        OriginSite::new("bnkHircPrefix", &ordinal, "objectRouting"),
        SourceUseRole::Selector,
    );
    let section = prepared.capture([section_routing(&section)])?;
    let position = prepared.capture([route])?;
    let extent = prepared.capture([
        route,
        (
            OriginSite::new("bnkHircObject", &body, "bodyLength"),
            SourceUseRole::Conversion,
        ),
    ])?;
    prepared.field("/data/payload/section_ordinal", section.clone())?;
    prepared.field("/subresource/indices/0", section)?;
    prepared.field("/data/payload/ordinal", position.clone())?;
    prepared.indices(position.clone())?;
    prepared.range("/data/payload/header_range", &position, &position)?;
    prepared.range("/data/payload/body_range", &position, &extent)?;
    for (path, field) in [
        ("/data/payload/type_code", "typeCode"),
        ("/data/payload/object_id", "objectId"),
    ] {
        let evidence = prepared.capture([
            route,
            (
                OriginSite::new("bnkHircObject", &body, field),
                SourceUseRole::Payload,
            ),
        ])?;
        prepared.field(path, evidence)?;
    }
    let kind = object_kind(&prepared, route, &body, version)?;
    prepared.field("/data/payload/kind", kind)?;
    Ok(prepared.finish())
}

fn object_kind(
    prepared: &BankOrigins<'_, '_, '_>,
    route: EvidenceSite<'_>,
    body: &[u64],
    version: Option<VersionSite>,
) -> Result<OriginContributions, MediaError> {
    let choice = (
        OriginSite::new("bnkDirectory", &[], "bankVersionSelection"),
        SourceUseRole::Selector,
    );
    version.map_or_else(
        || prepared.capture([route, choice]),
        |version| {
            let original = [version.body_offset];
            prepared.capture([
                route,
                choice,
                (
                    OriginSite::new("bnkHircObject", body, "typeCode"),
                    SourceUseRole::Conversion,
                ),
                (
                    OriginSite::new("bnkBkhd", &original, "version"),
                    SourceUseRole::Selector,
                ),
            ])
        },
    )
}

pub(crate) fn summary(
    input: &MediaOriginInput,
    section: &BankSourceSection,
    has_trailing: bool,
    writer: &MediaWriter<'_>,
) -> Result<PreparedRecordOrigins, MediaError> {
    let mut prepared = BankOrigins::new(input, writer)?;
    let ordinal = [section.ordinal()];
    let body = [section.body_offset()];
    let route = section_routing(&ordinal);
    let position = prepared.capture([route])?;
    prepared.field("/data/payload/section_ordinal", position.clone())?;
    prepared.indices(position)?;
    let count = prepared.capture([
        route,
        (
            OriginSite::new("bnkHircHeader", &body, "objectCount"),
            SourceUseRole::Payload,
        ),
    ])?;
    prepared.field("/data/payload/declared_objects", count)?;
    let prefix = (
        OriginSite::new("bnkHircPrefix", &ordinal, "completeObjectRouting"),
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
    if has_trailing {
        prepared.subtree("/issues", end)?;
    }
    Ok(prepared.finish())
}
