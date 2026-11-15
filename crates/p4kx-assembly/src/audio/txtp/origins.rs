// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};
use std::ops::Range;

use p4kx_extraction::{
    Archive, DecimalU64, FieldOrigin, FieldOrigins, FieldPath, InputScope, OriginArtifactBinding,
    OriginOutputAddress, SourceInput, SourceInventory, SourceLocation, SourceScope, SourceUse,
    SourceUseRole, SourceUses,
};

use crate::audio::ExportLoopOrigin;

use super::diagnostic::TxtpDiagnostic;
use super::loop_origin;
use super::render::RenderedLoop;

pub(super) struct OriginInputs<'a> {
    pub(super) diagnostics: &'a [TxtpDiagnostic<'a>],
    pub(super) loops: &'a [RenderedLoop<'a>],
}

/// The immediate origins and contributing sources for diagnosed and rendered TXTP values.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct TxtpOrigins {
    /// Field origins addressed relative to the report's file row.
    pub field_origins: FieldOrigins,
    /// Shared physical sources and observed source uses for these origins.
    pub provenance: SourceInventory,
    addresses: Vec<(OriginOutputAddress, FieldOrigin)>,
}

impl TxtpOrigins {
    pub(super) fn capture(
        archive: Option<&Archive>,
        inputs: &OriginInputs<'_>,
    ) -> io::Result<Self> {
        let sources = SourceUses::with_compact_use_refs();
        let mut field_origins = FieldOrigins::new();
        let mut addresses = Vec::with_capacity(inputs.diagnostics.len());
        for (index, diagnostic) in inputs.diagnostics.iter().enumerate() {
            add_field(
                &mut field_origins,
                &mut addresses,
                &format!("/approximations/{index}"),
                &diagnostic.header_bytes,
                diagnostic_origin(archive, &sources, diagnostic)?,
            )?;
        }
        for markers in inputs.loops {
            let source = markers.source;
            for (field, bytes, sites) in [
                (
                    "start",
                    &markers.start,
                    source.and_then(ExportLoopOrigin::start),
                ),
                ("end", &markers.end, source.and_then(ExportLoopOrigin::end)),
            ] {
                add_field(
                    &mut field_origins,
                    &mut addresses,
                    &format!("/commands/{}/loop/{field}", markers.command),
                    bytes,
                    loop_origin::field(archive, &sources, source, sites)?,
                )?;
            }
        }

        Ok(Self {
            field_origins,
            provenance: sources.freeze().map_err(io::Error::other)?,
            addresses,
        })
    }

    pub(super) fn write_companion(
        &self,
        writer: &mut dyn Write,
        binding: &OriginArtifactBinding,
    ) -> io::Result<()> {
        p4kx_extraction::write_origin_companion(
            writer,
            binding,
            &self.provenance,
            self.addresses
                .iter()
                .map(|(address, origin)| (address, origin)),
        )
    }
}

fn add_field(
    fields: &mut FieldOrigins,
    addresses: &mut Vec<(OriginOutputAddress, FieldOrigin)>,
    path: &str,
    bytes: &Range<usize>,
    origin: FieldOrigin,
) -> io::Result<()> {
    let path = FieldPath::parse(path).map_err(io::Error::other)?;
    fields
        .insert(path.clone(), origin.clone())
        .map_err(io::Error::other)?;
    let address = OriginOutputAddress::TextBytes {
        start: DecimalU64::new(offset(bytes.start)?),
        end_exclusive: DecimalU64::new(offset(bytes.end)?),
        field: path,
    };
    addresses.push((address, origin));
    Ok(())
}

fn diagnostic_origin(
    archive: Option<&Archive>,
    sources: &SourceUses,
    diagnostic: &TxtpDiagnostic<'_>,
) -> io::Result<FieldOrigin> {
    let (Some(archive), Some(source)) = (archive, diagnostic.source) else {
        return Ok(sources.origin(&[], false));
    };
    let Some(entry) = source.diagnostic_entry(archive) else {
        return Ok(sources.origin(&[], false));
    };
    let Some(site) = source.diagnostic_site(diagnostic.approximation) else {
        return Ok(sources.origin(&[], false));
    };

    let complete_ancestry = entry.ancestors().all(|ancestor| ancestor.is_some());
    let scope = sources
        .observe(SourceInput::ArchiveView(entry), InputScope::Payload)
        .map_err(io::Error::other)?;
    let mut actual = Vec::new();
    for word in site.words() {
        actual.push(source_use(
            &scope,
            word.namespace,
            word.field,
            word.bytes.clone(),
            SourceUseRole::Conversion,
        )?);
    }
    let version = source.version_bytes();
    let complete = complete_ancestry && version.is_some();
    if let Some(range) = version {
        let range = offset(range.start)?..offset(range.end)?;
        actual.push(source_use(
            &scope,
            "bnkBkhd",
            "version",
            range,
            SourceUseRole::Selector,
        )?);
    }
    Ok(sources.origin(&actual, complete))
}

fn source_use(
    scope: &SourceScope,
    namespace: &str,
    field: &str,
    range: Range<u64>,
    role: SourceUseRole,
) -> io::Result<SourceUse> {
    scope
        .at(
            SourceLocation::Subresource {
                namespace: namespace.to_owned(),
                indices: vec![range.start, range.end],
                field: field.to_owned(),
            },
            role,
        )
        .map_err(io::Error::other)
}

fn offset(position: usize) -> io::Result<u64> {
    u64::try_from(position).map_err(io::Error::other)
}
