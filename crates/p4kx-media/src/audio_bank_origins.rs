// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourceUseRole;
use p4kx_extraction::bank::BankSourceSection;

use crate::audio_record_origins as origins;
use crate::origin_capture::OriginSite;
use crate::origin_contract::OriginProfile;
use crate::origin_contributions::OriginContributions;
use crate::origin_input::MediaOriginInput;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::{MediaError, MediaWriter};

pub(crate) struct BankOrigins<'source, 'writer, 'run> {
    input: &'source MediaOriginInput,
    writer: &'writer MediaWriter<'run>,
    prepared: PreparedRecordOrigins,
}

pub(crate) type EvidenceSite<'a> = (OriginSite<'a>, SourceUseRole);

impl<'source, 'writer, 'run> BankOrigins<'source, 'writer, 'run> {
    pub(crate) fn new(
        input: &'source MediaOriginInput,
        writer: &'writer MediaWriter<'run>,
    ) -> Result<Self, MediaError> {
        Ok(Self {
            input,
            writer,
            prepared: origins::prepare_for(input, OriginProfile::AudioBank)?,
        })
    }

    pub(crate) fn capture<'site>(
        &self,
        sites: impl IntoIterator<Item = EvidenceSite<'site>>,
    ) -> Result<OriginContributions, MediaError> {
        origins::capture(self.input, sites, self.writer)
    }

    pub(crate) fn field(
        &mut self,
        path: &str,
        evidence: OriginContributions,
    ) -> Result<(), MediaError> {
        origins::field(&mut self.prepared, path, evidence, self.writer)
    }

    pub(crate) fn subtree(
        &mut self,
        path: &str,
        evidence: OriginContributions,
    ) -> Result<(), MediaError> {
        origins::subtree(&mut self.prepared, path, evidence, self.writer)
    }

    pub(crate) fn range(
        &mut self,
        path: &str,
        start: &OriginContributions,
        end: &OriginContributions,
    ) -> Result<(), MediaError> {
        origins::range(&mut self.prepared, path, start, end, self.writer)
    }

    pub(crate) fn indices(&mut self, evidence: OriginContributions) -> Result<(), MediaError> {
        origins::indices(&mut self.prepared, evidence, self.writer)
    }

    pub(crate) fn finish(self) -> PreparedRecordOrigins {
        self.prepared
    }
}

pub(crate) fn section(
    input: &MediaOriginInput,
    section: &BankSourceSection,
    writer: &MediaWriter<'_>,
) -> Result<PreparedRecordOrigins, MediaError> {
    let mut prepared = BankOrigins::new(input, writer)?;
    let ordinal = [section.ordinal()];
    let routing = section_routing(&ordinal);
    let body = [section.body_offset()];
    let tag_site = (
        OriginSite::new("bnkSection", &body, "tag"),
        SourceUseRole::Payload,
    );
    let directory = prepared.capture([routing])?;
    let extent = prepared.capture([
        routing,
        (
            OriginSite::new("bnkSection", &body, "bodyLength"),
            SourceUseRole::Conversion,
        ),
    ])?;
    let tag = prepared.capture([routing, tag_site])?;
    prepared.field("/data/payload/ordinal", directory.clone())?;
    prepared.field("/data/payload/tag", tag)?;
    for position in 0..section.id().0.len() {
        let at = [section.header_offset() + position as u64];
        let byte = prepared.capture([
            routing,
            (
                OriginSite::new("bnkByte", &at, "sectionTagByte"),
                SourceUseRole::Payload,
            ),
        ])?;
        prepared.field(&format!("/data/payload/tag/{position}"), byte)?;
    }
    let duplicate = prepared.capture([
        routing,
        (
            OriginSite::new("bnkSection", &body, "tag"),
            SourceUseRole::Selector,
        ),
        (
            OriginSite::new("bnkDirectoryPrefix", &ordinal, "uniqueSectionOccurrences"),
            SourceUseRole::Conversion,
        ),
    ])?;
    prepared.field("/data/payload/duplicate_unique", duplicate.clone())?;
    if section.is_duplicate_unique() {
        prepared.subtree("/issues", duplicate)?;
    }
    prepared.range("/data/payload/header_range", &directory, &directory)?;
    prepared.range("/data/payload/body_range", &directory, &extent)?;
    prepared.indices(directory)?;
    Ok(prepared.finish())
}

pub(crate) fn header(
    input: &MediaOriginInput,
    section: &BankSourceSection,
    supported: bool,
    writer: &MediaWriter<'_>,
) -> Result<PreparedRecordOrigins, MediaError> {
    let mut prepared = BankOrigins::new(input, writer)?;
    let ordinal = [section.ordinal()];
    let body = [section.body_offset()];
    let routing = section_routing(&ordinal);
    let directory = prepared.capture([routing])?;
    prepared.field("/data/payload/section_ordinal", directory.clone())?;
    prepared.indices(directory)?;
    for (path, field, role) in [
        ("/data/payload/version", "version", SourceUseRole::Payload),
        (
            "/data/payload/version_supported",
            "version",
            SourceUseRole::Conversion,
        ),
        ("/data/payload/bank_id", "bankId", SourceUseRole::Payload),
        (
            "/data/payload/language_id",
            "languageId",
            SourceUseRole::Payload,
        ),
    ] {
        let evidence =
            prepared.capture([routing, (OriginSite::new("bnkBkhd", &body, field), role)])?;
        prepared.field(path, evidence)?;
    }
    if !supported {
        let evidence = prepared.capture([
            routing,
            (
                OriginSite::new("bnkBkhd", &body, "version"),
                SourceUseRole::Conversion,
            ),
        ])?;
        prepared.subtree("/issues", evidence)?;
    }
    Ok(prepared.finish())
}

pub(crate) const fn section_routing(ordinal: &[u64]) -> EvidenceSite<'_> {
    (
        OriginSite::new("bnkDirectoryPrefix", ordinal, "sectionRouting"),
        SourceUseRole::Selector,
    )
}

pub(crate) const fn complete_directory(field: &'static str) -> EvidenceSite<'static> {
    (
        OriginSite::new("bnkDirectory", &[], field),
        SourceUseRole::Selector,
    )
}
