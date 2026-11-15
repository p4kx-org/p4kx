// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};
use std::ops::Range;

use p4kx_extraction::{
    Archive, DecimalU64, FieldOrigin, FieldOrigins, FieldPath, InputScope, OriginArtifactBinding,
    OriginOutputAddress, SourceInput, SourceInventory, SourceLocation, SourceScope, SourceUse,
    SourceUseRole, SourceUses,
};

use super::TxtpApproximation;
use super::diagnostic::TxtpDiagnostic;

/// The immediate origins and contributing sources for the report's approximation codes.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct TxtpOrigins {
    /// Code origins addressed relative to the report's file row.
    pub field_origins: FieldOrigins,
    /// Shared physical sources and observed source uses for these origins.
    pub provenance: SourceInventory,
    addresses: Vec<(OriginOutputAddress, FieldOrigin)>,
}

impl TxtpOrigins {
    pub(super) fn capture(
        archive: Option<&Archive>,
        diagnostics: &[TxtpDiagnostic<'_>],
    ) -> io::Result<Self> {
        let sources = SourceUses::with_compact_use_refs();
        let mut field_origins = FieldOrigins::new();
        let mut addresses = Vec::with_capacity(diagnostics.len());

        for (index, diagnostic) in diagnostics.iter().enumerate() {
            let path =
                FieldPath::parse(&format!("/approximations/{index}")).map_err(io::Error::other)?;
            let origin = diagnostic_origin(archive, &sources, diagnostic)?;
            field_origins
                .insert(path.clone(), origin.clone())
                .map_err(io::Error::other)?;
            let address = OriginOutputAddress::TextBytes {
                start: DecimalU64::new(offset(diagnostic.header_bytes.start)?),
                end_exclusive: DecimalU64::new(offset(diagnostic.header_bytes.end)?),
                field: path,
            };
            addresses.push((address, origin));
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

fn diagnostic_origin(
    archive: Option<&Archive>,
    sources: &SourceUses,
    diagnostic: &TxtpDiagnostic<'_>,
) -> io::Result<FieldOrigin> {
    let (namespace, field) = match diagnostic.approximation {
        TxtpApproximation::OmitContainerTransitions => {
            ("bnkHircRandomSequence", "randomSequenceTransitionMode")
        }
        TxtpApproximation::OmitBlendCurves => ("bnkHircBlendContainer", "blendPointCount"),
        _ => return Ok(sources.origin(&[], false)),
    };
    let (Some(archive), Some(source)) = (archive, diagnostic.source) else {
        return Ok(sources.origin(&[], false));
    };
    let Some(entry) = source.diagnostic_entry(archive) else {
        return Ok(sources.origin(&[], false));
    };
    let Some(range) = source.diagnostic_bytes(diagnostic.approximation) else {
        return Ok(sources.origin(&[], false));
    };

    let complete_ancestry = entry.ancestors().all(|ancestor| ancestor.is_some());
    let scope = sources
        .observe(SourceInput::ArchiveView(entry), InputScope::Payload)
        .map_err(io::Error::other)?;
    let mut actual = vec![source_use(
        &scope,
        namespace,
        field,
        range,
        SourceUseRole::Conversion,
    )?];
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
