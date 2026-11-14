// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourceUseRole;
use p4kx_extraction::bank::{BankHircObject, BankSourceSection};

use crate::audio_bank_inventory::VersionSite;
use crate::audio_bank_origins::{BankOrigins, EvidenceSite, complete_directory, section_routing};
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
    let sites = ObjectSites::new(object);
    let route = sites.route();
    let section = prepared.capture([section_routing(&sites.section)])?;
    let position = prepared.capture([route])?;
    let extent = prepared.capture([route, sites.extent(SourceUseRole::Conversion)])?;
    prepared.field("/data/payload/section_ordinal", section)?;
    prepared.field("/data/payload/ordinal", position.clone())?;
    sites.indices_origin(&mut prepared, position.clone())?;
    prepared.range("/data/payload/header_range", &position, &position)?;
    prepared.range("/data/payload/body_range", &position, &extent)?;
    for (path, field) in [
        ("/data/payload/type_code", "typeCode"),
        ("/data/payload/object_id", "objectId"),
    ] {
        let evidence = prepared.capture([
            route,
            (
                OriginSite::new("bnkHircObject", &sites.body, field),
                SourceUseRole::Payload,
            ),
        ])?;
        prepared.field(path, evidence)?;
    }
    let kind = object_kind(&prepared, route, &sites.body, version)?;
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

pub(crate) const fn inventory_sites() -> [EvidenceSite<'static>; 2] {
    [
        complete_directory("hircSections"),
        (
            OriginSite::new("bnkHircInventory", &[], "observedObjectEnvelopes"),
            SourceUseRole::Selector,
        ),
    ]
}

pub(crate) struct ObjectSites {
    section: [u64; 1],
    object: [u64; 2],
    body: [u64; 1],
}

impl ObjectSites {
    pub(crate) const fn new(object: &BankHircObject) -> Self {
        Self {
            section: [object.section_ordinal()],
            object: [object.section_ordinal(), object.ordinal()],
            body: [object.body_offset()],
        }
    }

    pub(crate) const fn indices(&self) -> [u64; 2] {
        self.object
    }

    pub(crate) const fn route(&self) -> EvidenceSite<'_> {
        (
            OriginSite::new("bnkHircPrefix", &self.object, "objectRouting"),
            SourceUseRole::Selector,
        )
    }

    pub(crate) const fn target_sites(&self) -> [EvidenceSite<'_>; 3] {
        [
            self.route(),
            (
                OriginSite::new("bnkHircObject", &self.body, "objectId"),
                SourceUseRole::Selector,
            ),
            (
                OriginSite::new("bnkHircObject", &self.body, "typeCode"),
                SourceUseRole::Payload,
            ),
        ]
    }

    pub(crate) fn target_fields(
        &self,
        prepared: &mut BankOrigins<'_, '_, '_>,
        evidence: &OriginContributions,
    ) -> Result<(), MediaError> {
        let section = prepared.capture([section_routing(&self.section)])?;
        let position = prepared.capture([self.route()])?;
        prepared.field("/data/payload/target_section_ordinal", section)?;
        prepared.field("/data/payload/target_object_ordinal", position)?;
        prepared.field("/data/payload/target_type_code", evidence.clone())
    }

    pub(crate) const fn extent(&self, role: SourceUseRole) -> EvidenceSite<'_> {
        (
            OriginSite::new("bnkHircObject", &self.body, "bodyLength"),
            role,
        )
    }

    pub(crate) const fn versioned<'a>(&'a self, version: &'a [u64; 1]) -> [EvidenceSite<'a>; 4] {
        [
            self.route(),
            complete_directory("bankVersionSelection"),
            (
                OriginSite::new("bnkHircObject", &self.body, "typeCode"),
                SourceUseRole::Selector,
            ),
            (
                OriginSite::new("bnkBkhd", version, "version"),
                SourceUseRole::Selector,
            ),
        ]
    }

    pub(crate) fn indices_origin(
        &self,
        prepared: &mut BankOrigins<'_, '_, '_>,
        position: OriginContributions,
    ) -> Result<(), MediaError> {
        prepared.indices(position)?;
        let section = prepared.capture([section_routing(&self.section)])?;
        prepared.field("/subresource/indices/0", section)
    }
}
