// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;
use std::path::{Path, PathBuf};

use p4kx_texture::{MergeReport, MergeSpan, MergeSpanSite};

use super::merge_inputs::{self, Inputs, WrittenMerge};
use crate::output_file::write_error;
use crate::{
    ConversionError, DecimalU64, FieldOrigin, FieldOrigins, FieldPath, FileReceipt, InputScope,
    OriginOutputAddress, SourceInput, SourceInventory, SourceLocation, SourceUse, SourceUseRole,
    SourceUses, Texture, write_origin_companion,
};

/// Exact source evidence for a merged DDS and its summary.
#[derive(Debug)]
pub struct MergeOrigins {
    /// The digest-bound DDS companion.
    pub receipt: PathBuf,
    /// Sources of the merge report's derived values.
    pub fields: FieldOrigins,
    /// The summary's shared source inventory.
    pub inventory: SourceInventory,
}

impl Texture<'_> {
    /// Writes a standard DDS and its byte-bound origin companion.
    ///
    /// # Errors
    /// Returns an original merge, input conflict or protected publication failure.
    pub fn merge_to_file_with_origins(
        &self,
        out: &Path,
    ) -> Result<(MergeReport, MergeOrigins), ConversionError> {
        self.refuse_input(out)?;
        let protection = self.write_protection()?;
        let receipt = FileReceipt::new(out, protection, protection)
            .map_err(|error| write_error(out, error))?;
        self.refuse_input(receipt.path())?;
        let mut merge_error = None;
        let written = receipt.publish(
            |writer| {
                let written = merge_inputs::write(self, writer).map_err(|error| {
                    let message = error.to_string();
                    merge_error = Some(error);
                    io::Error::other(message)
                })?;
                let origins = RecordedMerge::capture(&written)?;
                Ok((written.report, origins))
            },
            |writer, binding, (_, origins)| {
                write_origin_companion(
                    writer,
                    binding,
                    &origins.inventory,
                    origins
                        .output
                        .iter()
                        .map(|(address, field)| (address, field)),
                )
            },
        );
        let (report, origins) =
            written.map_err(|error| merge_error.unwrap_or_else(|| write_error(out, error)))?;
        Ok((
            report,
            MergeOrigins {
                receipt: receipt.path().to_path_buf(),
                fields: origins.summary,
                inventory: origins.summary_inventory,
            },
        ))
    }
}

struct RecordedMerge {
    output: Vec<(OriginOutputAddress, FieldOrigin)>,
    summary: FieldOrigins,
    summary_inventory: SourceInventory,
    inventory: SourceInventory,
}

impl RecordedMerge {
    fn capture(written: &WrittenMerge<'_>) -> io::Result<Self> {
        let uses = SourceUses::with_compact_use_refs();
        let header = written.inputs.header();
        let (header_span, mips) = written
            .spans
            .split_first()
            .filter(|(span, _)| span.site == MergeSpanSite::Header)
            .ok_or_else(|| io::Error::other("merged DDS header span is missing"))?;
        let header_len = header_span.byte_length;
        let original = observed_span(
            &uses,
            header,
            "ddsHeader",
            header_span.input_start,
            header_len,
            SourceUseRole::Payload,
        )?;
        let selector = observed_span(
            &uses,
            header,
            "ddsHeader",
            header_span.input_start,
            header_len,
            SourceUseRole::Selector,
        )?;
        let mut output = Vec::new();
        output
            .try_reserve(written.spans.len())
            .map_err(io::Error::other)?;
        output.push((
            OriginOutputAddress::DdsHeaderBytes {
                end_exclusive: DecimalU64::new(header_len),
            },
            uses.origin(&[original], known(header)),
        ));
        for mip in mips {
            output.push(mip_field(&uses, &written.inputs, header, &selector, mip)?);
        }
        let (summary, summary_inventory) = summary(header, header_len)?;
        Ok(Self {
            output,
            summary,
            summary_inventory,
            inventory: uses.freeze().map_err(io::Error::other)?,
        })
    }
}

fn mip_field(
    uses: &SourceUses,
    inputs: &Inputs<'_>,
    header: SourceInput<'_>,
    selector: &SourceUse,
    span: &MergeSpan,
) -> io::Result<(OriginOutputAddress, FieldOrigin)> {
    let MergeSpanSite::Mip {
        level,
        surface,
        part,
    } = span.site
    else {
        return Err(io::Error::other("merged DDS yielded an unexpected span"));
    };
    let source = part.map_or(header, |index| inputs.part(index, surface));
    let bytes = observed_span(
        uses,
        source,
        "ddsMip",
        span.input_start,
        span.byte_length,
        SourceUseRole::Payload,
    )?;
    Ok((
        OriginOutputAddress::DdsMipBytes {
            level,
            surface: DecimalU64::new(surface),
            start: DecimalU64::new(span.output_start),
            end_exclusive: DecimalU64::new(span.output_start + span.byte_length),
        },
        uses.origin(&[bytes, selector.clone()], known(header) && known(source)),
    ))
}

fn summary(
    header: SourceInput<'_>,
    header_len: u64,
) -> io::Result<(FieldOrigins, SourceInventory)> {
    let uses = SourceUses::with_compact_use_refs();
    let scope = match header {
        SourceInput::ArchiveView(_) => InputScope::ArchiveDirectoryMetadata,
        _ => InputScope::FileMetadata,
    };
    let meta = uses.observe(header, scope).map_err(io::Error::other)?;
    let path = meta
        .at(
            SourceLocation::Metadata {
                field: if matches!(header, SourceInput::ArchiveView(_)) {
                    "path"
                } else {
                    "name"
                }
                .into(),
            },
            SourceUseRole::Payload,
        )
        .map_err(io::Error::other)?;
    let length = meta
        .at(
            SourceLocation::Metadata {
                field: if matches!(header, SourceInput::ArchiveView(_)) {
                    "uncompressedSize"
                } else {
                    "byteLength"
                }
                .into(),
            },
            SourceUseRole::Conversion,
        )
        .map_err(io::Error::other)?;
    let dds = observed_span(
        &uses,
        header,
        "ddsHeader",
        0,
        header_len,
        SourceUseRole::Conversion,
    )?;
    let layout = uses.origin(&[dds, length], known(header));

    let mut result = FieldOrigins::new();
    for (key, origin) in [
        ("/texture", uses.origin(&[path], known(header))),
        ("/parts", layout.clone()),
        ("/size", layout),
    ] {
        result
            .insert(FieldPath::parse(key).map_err(io::Error::other)?, origin)
            .map_err(io::Error::other)?;
    }
    Ok((result, uses.freeze().map_err(io::Error::other)?))
}

fn observed_span(
    uses: &SourceUses,
    input: SourceInput<'_>,
    namespace: &str,
    start: u64,
    length: u64,
    role: SourceUseRole,
) -> io::Result<SourceUse> {
    uses.observe(input, InputScope::Payload)
        .and_then(|scope| {
            scope.at(
                SourceLocation::Subresource {
                    namespace: namespace.to_owned(),
                    indices: vec![start, length],
                    field: "bytes".into(),
                },
                role,
            )
        })
        .map_err(io::Error::other)
}

const fn known(input: SourceInput<'_>) -> bool {
    !matches!(input, SourceInput::Unknown)
}
