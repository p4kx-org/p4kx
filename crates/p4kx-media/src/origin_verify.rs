// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use crate::checkpoint::Checkpoint;
use crate::origin_contract::{
    OriginProfile, OriginReceipts, OriginScope, OriginStream, OriginStreamSummary,
};
use crate::origin_coverage_read::Coverage;
use crate::origin_inventory_read::ReadInventory;
use crate::origin_line_reader::OriginLines;
use crate::origin_payload_read::PayloadStreams;
use crate::origin_read_policy::OriginReadPolicy;
use crate::origin_rows_read::{ReadRow, RowSeed};
use crate::{Index, MediaError, MediaKind, origin_index, origin_json, origin_stream};

/// Presence of the separately hashed current mapping protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OriginAvailability {
    HistoricalUnknown,
    Current,
}

/// Verifies origin descriptions and their exact actual payload occurrence coverage.
pub(crate) fn generation(
    root: &Path,
    index: &Index,
    checkpoint: Checkpoint<'_>,
    policy: OriginReadPolicy,
) -> Result<OriginAvailability, MediaError> {
    let Some(receipts) = &index.field_origins else {
        return Ok(OriginAvailability::HistoricalUnknown);
    };
    origin_index::validate_receipts(receipts)?;
    let inventory = ReadInventory::open(root, receipts, checkpoint, policy)?;
    let mut payloads = PayloadStreams::open(root, index, checkpoint)?;
    let mut lines = OriginLines::open(root, &receipts.fields, checkpoint, policy.line_bytes)?;
    loop {
        match lines.next(RowSeed(policy.fields))? {
            Some(ReadRow::Occurrence {
                scope,
                profile,
                payload_sha256,
            }) => {
                verify_profile(scope.stream, profile)?;
                let bytes = payloads.occurrence(&scope, &payload_sha256, checkpoint)?;
                let fields = occurrence(&scope, &bytes, index, &inventory, &mut lines, policy)?;
                payloads.fields(scope.stream, fields)?;
            }
            Some(ReadRow::Eof { streams }) => {
                return finish(receipts, &streams, lines, &mut payloads, checkpoint, policy);
            }
            _ => {
                return Err(invalid(
                    "origin stream lacks its occurrence header or terminal EOF",
                ));
            }
        }
    }
}

fn finish(
    receipts: &OriginReceipts,
    streams: &[OriginStreamSummary],
    mut lines: OriginLines<'_, '_>,
    payloads: &mut PayloadStreams,
    checkpoint: Checkpoint<'_>,
    policy: OriginReadPolicy,
) -> Result<OriginAvailability, MediaError> {
    if streams != receipts.streams.as_slice() || lines.next(RowSeed(policy.fields))?.is_some() {
        return Err(invalid(
            "origin EOF or trailing rows contradict the receipt group",
        ));
    }
    payloads.finish(streams, checkpoint)?;
    lines.finish()?;
    Ok(OriginAvailability::Current)
}
fn occurrence(
    scope: &OriginScope,
    bytes: &[u8],
    index: &Index,
    inventory: &ReadInventory,
    lines: &mut OriginLines<'_, '_>,
    policy: OriginReadPolicy,
) -> Result<u64, MediaError> {
    let mut coverage = Coverage::new();
    let mut fields = 0_usize;
    let depth = origin_stream::payload_depth(scope.stream, index.limits.value_depth);
    origin_json::visit(bytes, depth, &mut |path| {
        if fields >= policy.fields.fields {
            return Err(invalid("origin field count exceeds the reader policy"));
        }
        fields = fields
            .checked_add(1)
            .ok_or_else(|| invalid("origin field count overflow"))?;
        let address = origin_stream::address(scope.stream, path);
        field(
            scope,
            address.as_ref(),
            inventory,
            lines,
            policy,
            &mut coverage,
        )
    })?;
    match lines.next(RowSeed(policy.fields))? {
        Some(ReadRow::End {
            scope: actual_scope,
            coverage: expected,
        }) if &actual_scope == scope => coverage.finish(&expected),
        _ => Err(invalid(
            "origin occurrence lacks its matching completion row",
        )),
    }
}

fn field(
    scope: &OriginScope,
    path: &str,
    inventory: &ReadInventory,
    lines: &mut OriginLines<'_, '_>,
    policy: OriginReadPolicy,
    coverage: &mut Coverage,
) -> Result<(), MediaError> {
    let Some(ReadRow::Field {
        scope: actual_scope,
        field,
        origin,
    }) = lines.next(RowSeed(policy.fields))?
    else {
        return Err(invalid(
            "actual payload node lacks its immediate field origin",
        ));
    };
    if &actual_scope != scope || field != path {
        return Err(invalid(
            "field origin scope or absolute address disagrees with actual payload",
        ));
    }
    inventory.validate_origin(&origin)?;
    coverage.observe(&origin)
}

fn verify_profile(stream: OriginStream, profile: OriginProfile) -> Result<(), MediaError> {
    let valid = matches!(
        (stream, profile),
        (
            OriginStream::Records { .. } | OriginStream::Sources | OriginStream::Artifacts,
            OriginProfile::Unmigrated
        ) | (OriginStream::IndexBuild, OriginProfile::IndexBuild)
            | (
                OriginStream::Records {
                    kind: MediaKind::Audio
                },
                OriginProfile::AudioWem | OriginProfile::AudioBank
            )
            | (
                OriginStream::Records {
                    kind: MediaKind::AudioControl
                },
                OriginProfile::AudioControl
            )
            | (
                OriginStream::Records {
                    kind: MediaKind::Material
                },
                OriginProfile::MaterialLibrary
                    | OriginProfile::MaterialFailure
                    | OriginProfile::MaterialSourceLimit
                    | OriginProfile::MaterialConfigurationFailure
            )
            | (
                OriginStream::Records {
                    kind: MediaKind::Archive
                },
                OriginProfile::ArchiveEntry
                    | OriginProfile::ArchiveLayout
                    | OriginProfile::ArchiveBuild,
            )
            | (
                OriginStream::Records {
                    kind: MediaKind::Shader
                },
                OriginProfile::ShaderDefinition
                    | OriginProfile::ShaderProperty
                    | OriginProfile::ShaderDeclaration
                    | OriginProfile::ShaderFailure
                    | OriginProfile::ShaderSourceLimit
                    | OriginProfile::ShaderConfigurationFailure,
            )
    );
    if !valid {
        return Err(invalid("origin profile does not own this payload stream"));
    }
    Ok(())
}

fn invalid(message: &str) -> MediaError {
    MediaError::Destination(message.to_owned())
}

#[cfg(test)]
#[path = "origin_verify_tests.rs"]
mod tests;
