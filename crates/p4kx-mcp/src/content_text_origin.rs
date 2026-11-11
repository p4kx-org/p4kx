// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_extraction::cryxml::XmlWriteField;
use p4kx_extraction::{
    Archive, Entry, FieldOrigin, ObservedEntryText, SourceEvidenceError, SourceInventory,
    SourceUseRole, TextReadEvidence,
};
use schemars::JsonSchema;
use serde::Serialize;

use crate::content_origin::EntryPageSources;

#[derive(Debug, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct TextFieldOrigins {
    #[serde(rename = "/structuredContent/archive")]
    archive: FieldOrigin,
    #[serde(rename = "/structuredContent/path")]
    path: FieldOrigin,
    #[serde(rename = "/structuredContent/size")]
    size: FieldOrigin,
    #[serde(rename = "/structuredContent/offset")]
    offset: FieldOrigin,
    #[serde(rename = "/structuredContent/length")]
    length: FieldOrigin,
    #[serde(rename = "/structuredContent/encoding")]
    encoding: FieldOrigin,
    #[serde(rename = "/structuredContent/text")]
    text: FieldOrigin,
    #[serde(rename = "/structuredContent/more")]
    more: FieldOrigin,
    #[serde(
        rename = "/structuredContent/next_offset",
        skip_serializing_if = "Option::is_none"
    )]
    next_offset: Option<FieldOrigin>,
    #[serde(rename = "/structuredContent/clamped")]
    clamped: FieldOrigin,
    #[serde(rename = "/structuredContent/max_bytes")]
    max_bytes: FieldOrigin,
}

impl TextFieldOrigins {
    pub(crate) fn capture(
        archive: &Archive,
        entry: &Entry,
        result: &ObservedEntryText,
    ) -> Result<(Self, SourceInventory), SourceEvidenceError> {
        let sources = EntryPageSources::observe(archive, entry)?;
        let page = TextPageOrigins::capture(&sources, &result.evidence)?;
        let local = sources.local();
        let fields = Self {
            archive: local.clone(),
            path: sources.path,
            size: page.size,
            offset: page.offset,
            length: page.length,
            encoding: page.encoding,
            text: page.text,
            more: page.navigation.clone(),
            next_offset: result.page.next_offset().map(|_| page.navigation),
            clamped: local.clone(),
            max_bytes: local,
        };
        Ok((fields, sources.uses.freeze()?))
    }
}

struct TextPageOrigins {
    size: FieldOrigin,
    offset: FieldOrigin,
    length: FieldOrigin,
    encoding: FieldOrigin,
    text: FieldOrigin,
    navigation: FieldOrigin,
}

impl TextPageOrigins {
    fn capture(
        sources: &EntryPageSources,
        evidence: &TextReadEvidence,
    ) -> Result<Self, SourceEvidenceError> {
        match evidence {
            TextReadEvidence::Utf8 {
                prefix,
                bom_probe,
                start_probe,
                inspected,
                payload,
                ..
            } => Self::utf8(sources, prefix, bom_probe, start_probe, inspected, payload),
            TextReadEvidence::Transcoded {
                input, xml_fields, ..
            } => Self::transcoded(sources, input, xml_fields),
            _ => Ok(Self::from_origin(sources.uses.origin(&[], false))),
        }
    }

    fn utf8(
        sources: &EntryPageSources,
        prefix: &Range<u64>,
        bom_probe: &Range<u64>,
        start_probe: &Range<u64>,
        inspected: &Range<u64>,
        payload: &Range<u64>,
    ) -> Result<Self, SourceEvidenceError> {
        use SourceUseRole::{Payload, Selector};

        let size = sources.stored_size()?;
        let prefix = sources.decoded_range(prefix.clone(), "textEncoding", Selector)?;
        let bom = sources.decoded_range(bom_probe.clone(), "byteOrderMark", Selector)?;
        let alignment = sources.decoded_range(start_probe.clone(), "startAlignment", Selector)?;
        let boundaries =
            sources.decoded_range(inspected.clone(), "characterBoundaries", Selector)?;
        let bytes = sources.decoded_range(payload.clone(), "utf8Text", Payload)?;
        let mut extent = vec![size];
        extent.extend(bom.iter().cloned());
        let size = sources.origin(&extent);
        let encoding = sources.origin(prefix.as_slice());
        let offset = sources.origin(alignment.as_slice());
        let mut written = Vec::from(bytes.as_slice());
        written.extend(boundaries);
        written.extend(bom);
        let text = sources.origin(&written);
        let length = text.clone();
        extent.extend(written);
        Ok(Self {
            size,
            offset,
            length,
            encoding,
            text,
            navigation: sources.origin(&extent),
        })
    }

    fn transcoded(
        sources: &EntryPageSources,
        input: &Range<u64>,
        fields: &[XmlWriteField],
    ) -> Result<Self, SourceEvidenceError> {
        let converted =
            sources.decoded_range(input.clone(), "transcodeInput", SourceUseRole::Conversion)?;
        let mut result = Self::from_origin(sources.origin(converted.as_slice()));
        let mut actual = Vec::from(converted.as_slice());
        let mut complete = true;
        for field in fields {
            match sources.cryxml_field(field)? {
                Some(captured) => actual.extend(captured.uses),
                None => complete = false,
            }
        }
        result.text = sources.origin_when(&actual, complete);
        Ok(result)
    }

    fn from_origin(origin: FieldOrigin) -> Self {
        Self {
            size: origin.clone(),
            offset: origin.clone(),
            length: origin.clone(),
            encoding: origin.clone(),
            text: origin.clone(),
            navigation: origin,
        }
    }
}
