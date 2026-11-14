// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourceUseRole;
use p4kx_extraction::bank::{
    BankDeclaredLengthSource, BankDidxEntry, BankMediaClassSource, BankMediaInfo, BankSourceSection,
};

use crate::audio_bank_didx_index::DidxSite;
use crate::audio_bank_media::DataResolution;
use crate::audio_bank_origins::{BankOrigins, EvidenceSite, complete_directory, section_routing};
use crate::audio_record_origins as origins;
use crate::origin_capture::OriginSite;
use crate::origin_contributions::OriginContributions;
use crate::origin_input::MediaOriginInput;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::{MediaError, MediaWriter};

pub(crate) struct EmbeddedWemOrigins {
    pub(crate) section: OriginContributions,
    pub(crate) triplet: OriginContributions,
    pub(crate) selection: OriginContributions,
}

pub(crate) fn embedded_wem(
    input: &MediaOriginInput,
    entry: &BankDidxEntry,
    section: &BankSourceSection,
    info: &BankMediaInfo,
    writer: &MediaWriter<'_>,
) -> Result<EmbeddedWemOrigins, MediaError> {
    let section_ordinal = [entry.section_ordinal()];
    let ordinal = [entry.section_ordinal(), entry.ordinal()];
    let triplet_offset = [entry.triplet_offset()];
    let routing = section_routing(&section_ordinal);
    let route = (
        OriginSite::new("bnkDidxPrefix", &ordinal, "tripletRouting"),
        SourceUseRole::Selector,
    );
    let selected = data_selection(route, &triplet_offset, section);
    let start = [info.range.start];
    let prefix = ["signature", "form", "riffSize"].map(|field| {
        (
            OriginSite::new("bnkMediaHeader", &start, field),
            SourceUseRole::Conversion,
        )
    });
    Ok(EmbeddedWemOrigins {
        section: origins::capture(input, [routing], writer)?,
        triplet: origins::capture(input, [routing, route], writer)?,
        selection: origins::capture(
            input,
            std::iter::once(routing)
                .chain(selected.sites())
                .chain(prefix),
            writer,
        )?,
    })
}

pub(crate) fn prepare(
    input: &MediaOriginInput,
    entry: &BankDidxEntry,
    resolution: &DataResolution<'_>,
    first: Option<&DidxSite>,
    writer: &MediaWriter<'_>,
) -> Result<PreparedRecordOrigins, MediaError> {
    let mut prepared = BankOrigins::new(input, writer)?;
    let section_ordinal = [entry.section_ordinal()];
    let ordinal = [entry.section_ordinal(), entry.ordinal()];
    let triplet = [entry.triplet_offset()];
    let route = (
        OriginSite::new("bnkDidxPrefix", &ordinal, "tripletRouting"),
        SourceUseRole::Selector,
    );
    let position = prepared.capture([section_routing(&section_ordinal), route])?;
    let section = prepared.capture([section_routing(&section_ordinal)])?;
    prepared.field("/data/payload/section_ordinal", section.clone())?;
    prepared.field("/subresource/indices/0", section)?;
    prepared.field("/data/payload/ordinal", position.clone())?;
    prepared.indices(position.clone())?;
    prepared.range("/data/payload/index_range", &position, &position)?;
    for (path, field) in [
        ("/data/payload/media_id", "mediaId"),
        ("/data/payload/data_offset", "dataOffset"),
        ("/data/payload/data_length", "dataLength"),
    ] {
        let evidence = prepared.capture([
            route,
            (
                OriginSite::new("bnkDidx", &triplet, field),
                SourceUseRole::Payload,
            ),
        ])?;
        prepared.field(path, evidence)?;
    }
    let data_issue = data_origins(&mut prepared, route, &triplet, resolution)?;
    let duplicate = first
        .map(|first| {
            prepared.capture([
                section_routing(&[first.section]),
                section_routing(&section_ordinal),
                (
                    OriginSite::new("bnkDidx", &[first.offset], "mediaId"),
                    SourceUseRole::Conversion,
                ),
                (
                    OriginSite::new("bnkDidx", &triplet, "mediaId"),
                    SourceUseRole::Conversion,
                ),
            ])
        })
        .transpose()?;
    issue_origins(&mut prepared, writer, duplicate, data_issue)?;
    Ok(prepared.finish())
}

fn issue_origins(
    prepared: &mut BankOrigins<'_, '_, '_>,
    writer: &MediaWriter<'_>,
    duplicate: Option<OriginContributions>,
    data: Option<OriginContributions>,
) -> Result<(), MediaError> {
    match (duplicate, data) {
        (Some(duplicate), Some(data)) => {
            let mut combined = writer.capture_origins();
            combined.include(&duplicate, writer)?;
            combined.include(&data, writer)?;
            prepared.field("/issues", combined.finish())?;
            prepared.subtree("/issues/0", duplicate)?;
            prepared.subtree("/issues/1", data)
        }
        (Some(issue), None) | (None, Some(issue)) => prepared.subtree("/issues", issue),
        (None, None) => Ok(()),
    }
}

fn data_origins(
    prepared: &mut BankOrigins<'_, '_, '_>,
    route: EvidenceSite<'_>,
    triplet: &[u64],
    resolution: &DataResolution<'_>,
) -> Result<Option<OriginContributions>, MediaError> {
    match resolution {
        DataResolution::Resolved { section, info } => {
            resolved(prepared, route, triplet, section, info)?;
            Ok(None)
        }
        DataResolution::OutsideData { section } => {
            let sites = data_selection(route, triplet, section);
            let evidence = prepared.capture(sites.sites())?;
            prepared.field("/data/payload/data", evidence.clone())?;
            prepared.field("/data/payload/data/status", evidence.clone())?;
            prepared.field("/data/payload/data/section_ordinal", evidence.clone())?;
            prepared.field("/data/payload/data/data_length", evidence.clone())?;
            Ok(Some(evidence))
        }
        other => {
            let inventory = match other {
                DataResolution::Unavailable => (
                    OriginSite::new("bnkDirectory", &[], "incompleteDataSectionInventory"),
                    SourceUseRole::Conversion,
                ),
                _ => complete_directory("dataSectionOccurrences"),
            };
            let evidence = prepared.capture([route, inventory])?;
            prepared.field("/data/payload/data", evidence.clone())?;
            prepared.field("/data/payload/data/status", evidence.clone())?;
            if matches!(other, DataResolution::Ambiguous(_)) {
                prepared.field("/data/payload/data/candidates", evidence)?;
                return Ok(None);
            }
            Ok(matches!(other, DataResolution::Missing).then_some(evidence))
        }
    }
}

fn resolved(
    prepared: &mut BankOrigins<'_, '_, '_>,
    route: EvidenceSite<'_>,
    triplet: &[u64],
    section: &BankSourceSection,
    info: &BankMediaInfo,
) -> Result<(), MediaError> {
    let sites = data_selection(route, triplet, section);
    let extent = prepared.capture(sites.sites())?;
    prepared.field("/data/payload/data", extent.clone())?;
    prepared.field("/data/payload/data/status", extent.clone())?;
    prepared.field("/data/payload/data/section_ordinal", extent.clone())?;
    prepared.range("/data/payload/data/range", &extent, &extent)?;
    prefix_origins(prepared, &sites.sites(), info)
}

fn prefix_origins(
    prepared: &mut BankOrigins<'_, '_, '_>,
    selection: &[EvidenceSite<'_>],
    info: &BankMediaInfo,
) -> Result<(), MediaError> {
    let start = [info.range.start];
    let signature = (
        OriginSite::new("bnkMediaHeader", &start, "signature"),
        SourceUseRole::Conversion,
    );
    let mut class_sites = selection.to_vec();
    match info.class_source {
        BankMediaClassSource::TooShort => {}
        BankMediaClassSource::Signature | BankMediaClassSource::MissingForm => {
            class_sites.push(signature);
        }
        BankMediaClassSource::SignatureAndForm => {
            class_sites.push(signature);
            class_sites.push((
                OriginSite::new("bnkMediaHeader", &start, "form"),
                SourceUseRole::Conversion,
            ));
        }
        _ => {
            return Err(MediaError::Contract(
                "unrecognized bank media class evidence".into(),
            ));
        }
    }
    let class = prepared.capture(class_sites)?;
    prepared.field("/data/payload/data/class", class)?;

    let mut length_sites = selection.to_vec();
    match info.declared_len_source {
        BankDeclaredLengthSource::TooShort => {}
        BankDeclaredLengthSource::Signature => length_sites.push(signature),
        BankDeclaredLengthSource::SignatureAndLength => {
            length_sites.push(signature);
            length_sites.push((
                OriginSite::new("bnkMediaHeader", &start, "riffSize"),
                SourceUseRole::Conversion,
            ));
        }
        _ => {
            return Err(MediaError::Contract(
                "unrecognized bank media length evidence".into(),
            ));
        }
    }
    let length = prepared.capture(length_sites)?;
    prepared.field("/data/payload/data/declared_wem_length", length.clone())?;
    prepared.field("/data/payload/data/prefetch", length)
}

const fn data_selection<'site>(
    route: EvidenceSite<'site>,
    triplet: &'site [u64],
    section: &BankSourceSection,
) -> DataSelection<'site> {
    DataSelection {
        route,
        triplet,
        section_body: [section.body_offset()],
    }
}

struct DataSelection<'site> {
    route: EvidenceSite<'site>,
    triplet: &'site [u64],
    section_body: [u64; 1],
}

impl DataSelection<'_> {
    const fn sites(&self) -> [EvidenceSite<'_>; 5] {
        [
            self.route,
            complete_directory("uniqueDataSection"),
            (
                OriginSite::new("bnkDidx", self.triplet, "dataOffset"),
                SourceUseRole::Conversion,
            ),
            (
                OriginSite::new("bnkDidx", self.triplet, "dataLength"),
                SourceUseRole::Conversion,
            ),
            (
                OriginSite::new("bnkSection", &self.section_body, "bodyLength"),
                SourceUseRole::Selector,
            ),
        ]
    }
}
