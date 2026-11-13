// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::Read as _;
use std::path::Path;

use p4kx_extraction::{
    DdsHeader, DdsRawHeader, DdsRawHeaderInfo, DdsSourceInfo, DecimalU64, MAX_DDS_HEADER_LEN,
    TextureError,
};
use schemars::JsonSchema;
use serde::Serialize;

use super::family;
use super::origin::TextureHeaderOrigins;
use crate::required_reader::RequiredReader;
use crate::{MediaError, MediaIssue, MediaWriter, Severity, SourceContext, Status, Subresource};

pub(super) const NAMESPACE: &str = "texture.dds-header";

#[derive(Serialize, JsonSchema)]
#[serde(tag = "record_type", content = "payload", rename_all = "kebab-case")]
pub(super) enum TextureHeaderPayload {
    Header(Header),
    InvalidHeader(InvalidHeader),
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct Header {
    alpha_tail: bool,
    source_metadata: DdsSourceInfo,
    decoder: Decoder,
    split_parts: SplitParts,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
enum Decoder {
    Png,
    OpenExr,
    Unsupported,
}

#[derive(Serialize, JsonSchema)]
#[serde(tag = "status", rename_all = "kebab-case", deny_unknown_fields)]
enum SplitParts {
    Known { expected_count: u32 },
    UnsupportedLayout,
    InconsistentHeaderTail { tail_bytes: DecimalU64 },
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct InvalidHeader {
    alpha_tail: bool,
    header_source_bytes: DecimalU64,
    raw: Option<DdsRawHeaderInfo>,
}

#[derive(Clone, Copy)]
pub(super) struct HeaderRole<'a> {
    pub(super) family: &'a str,
    pub(super) alpha: bool,
}

pub(super) fn project(
    context: &SourceContext<'_>,
    writer: &mut MediaWriter<'_>,
    role: HeaderRole<'_>,
) -> Result<(), MediaError> {
    let input = context.origin_input(writer)?;
    let bytes = read_prefix(context, writer)?;
    let size = context.source.size;
    let parsed = DdsRawHeader::parse(&bytes);
    let origins = TextureHeaderOrigins::new(
        &input,
        context.entry.index(),
        inspected_prefix(&bytes, &parsed),
    );
    let subresource = Subresource::new(NAMESPACE, Vec::new());
    let (raw, detail) = match parsed {
        Ok(raw) => match DdsSourceInfo::from_raw_with_header(&raw, size) {
            Ok((metadata, normalized)) => {
                let header = Header::new(role.alpha, metadata, &normalized)?;
                let expected = header.split_parts.expected_count();
                origins
                    .valid(&raw, normalized.format().layout().is_some(), writer)?
                    .write(
                        writer,
                        &subresource,
                        Status::Complete,
                        &TextureHeaderPayload::Header(header),
                        &[],
                    )?;
                return family::project(
                    context,
                    writer,
                    role,
                    &origins,
                    expected.map(|count| (&raw, &normalized, count)),
                );
            }
            Err(error) => (Some(raw), error.to_string()),
        },
        Err(error) => (None, error.to_string()),
    };
    origins.invalid(raw.as_ref(), writer)?.write(
        writer,
        &subresource,
        Status::Partial,
        &TextureHeaderPayload::InvalidHeader(InvalidHeader {
            alpha_tail: role.alpha,
            header_source_bytes: DecimalU64::new(size),
            raw: raw.as_ref().map(DdsRawHeaderInfo::of),
        }),
        &[MediaIssue::new(
            "texture.invalid-header",
            "source_metadata",
            Severity::Warning,
            detail,
        )],
    )?;
    family::project(context, writer, role, &origins, None)
}

impl SplitParts {
    const fn expected_count(&self) -> Option<u32> {
        match self {
            Self::Known { expected_count } => Some(*expected_count),
            _ => None,
        }
    }
}

impl Header {
    fn new(
        alpha_tail: bool,
        source_metadata: DdsSourceInfo,
        normalized: &DdsHeader,
    ) -> Result<Self, MediaError> {
        let format = normalized.format();
        let decoder = if format.is_high_range() {
            Decoder::OpenExr
        } else if format.is_decodable() {
            Decoder::Png
        } else {
            Decoder::Unsupported
        };
        let split_parts =
            split_parts(normalized.part_count(source_metadata.header_source_bytes.get()))?;
        Ok(Self {
            alpha_tail,
            source_metadata,
            decoder,
            split_parts,
        })
    }
}

fn split_parts(result: Result<u32, TextureError>) -> Result<SplitParts, MediaError> {
    match result {
        Ok(expected_count) => Ok(SplitParts::Known { expected_count }),
        Err(TextureError::UnsupportedFormat(_)) => Ok(SplitParts::UnsupportedLayout),
        Err(TextureError::TailMismatch { tail }) => Ok(SplitParts::InconsistentHeaderTail {
            tail_bytes: DecimalU64::new(tail),
        }),
        Err(error) => Err(MediaError::Contract(error.to_string())),
    }
}

fn inspected_prefix(bytes: &[u8], result: &Result<DdsRawHeader, TextureError>) -> u64 {
    match result {
        Ok(raw) => raw.header_len() as u64,
        Err(TextureError::NotDds) => bytes.len().min(4) as u64,
        Err(TextureError::InvalidHeader(_)) => bytes.len().min(8) as u64,
        Err(_) => bytes.len() as u64,
    }
}

fn read_prefix(
    context: &SourceContext<'_>,
    writer: &MediaWriter<'_>,
) -> Result<Vec<u8>, MediaError> {
    let path = context.entry.path();
    let size = context.source.size;
    let len = usize::try_from(size.min(MAX_DDS_HEADER_LEN as u64))
        .map_err(|_| MediaError::Contract("DDS prefix length exceeds address width".into()))?;
    let mut bytes = vec![0; len];
    let mut reader = RequiredReader::open(
        context.payload,
        path,
        size,
        context.directory.input_state()?.budget,
        writer.checkpoint,
        writer.limits(),
    )?;
    reader
        .read_exact(&mut bytes)
        .map_err(|cause| crate::error::io(Path::new(path), cause))?;
    Ok(bytes)
}
