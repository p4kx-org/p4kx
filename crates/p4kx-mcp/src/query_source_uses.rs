// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::query::{QueryByteUse, QuerySource, RecordNameKind};
use p4kx_extraction::{SourceLocation, SourceUse, SourceUseRole};

use crate::forge_origins::{EvidenceError, ForgeOrigins, site};

pub(crate) fn record_site(
    origins: &ForgeOrigins,
    index: u32,
    field: &str,
    role: SourceUseRole,
) -> Result<SourceUse, EvidenceError> {
    Ok(origins.scope.at(
        SourceLocation::DataForge {
            record_index: index,
            field: field.to_owned(),
        },
        role,
    )?)
}

pub(crate) fn uses(
    origins: &ForgeOrigins,
    sources: impl IntoIterator<Item = QuerySource>,
    value_role: SourceUseRole,
) -> Result<Vec<SourceUse>, EvidenceError> {
    sources
        .into_iter()
        .filter_map(|source| bind_source(origins, source, value_role))
        .collect()
}

fn bind_source(
    origins: &ForgeOrigins,
    source: QuerySource,
    value_role: SourceUseRole,
) -> Option<Result<SourceUse, EvidenceError>> {
    Some(match source {
        QuerySource::Bytes { start, end, kind } => {
            let (label, role) = match kind {
                QueryByteUse::Value => ("slot", value_role),
                QueryByteUse::Text => ("text", value_role),
                QueryByteUse::Array => ("arrayHeader", SourceUseRole::Selector),
                QueryByteUse::Pointer => ("pointer", SourceUseRole::Selector),
                QueryByteUse::Reference => ("reference", SourceUseRole::Selector),
                QueryByteUse::WeakLink => ("weakLink", SourceUseRole::Selector),
                QueryByteUse::Unresolved => ("unresolvedLink", SourceUseRole::Selector),
                _ => return None,
            };
            site(
                &origins.scope,
                "dataforgeDecodedBytes",
                label,
                vec![start, end],
                role,
            )
            .map_err(Into::into)
        }
        QuerySource::Record { index } => {
            record_site(origins, index, "instance", SourceUseRole::Selector)
        }
        QuerySource::ArrayElement {
            header_start,
            index,
        } => site(
            &origins.scope,
            "dataforgeArrayElement",
            "selection",
            vec![header_start, u64::from(index)],
            SourceUseRole::Selector,
        )
        .map_err(Into::into),
        QuerySource::Instance {
            structure,
            instance,
        } => site(
            &origins.scope,
            "dataforgeInstance",
            "selection",
            vec![u64::from(structure), u64::from(instance)],
            SourceUseRole::Selector,
        )
        .map_err(Into::into),
        QuerySource::Structure { index } => site(
            &origins.scope,
            "dataforgeStructDefinition",
            "sourceType",
            vec![u64::from(index)],
            SourceUseRole::Selector,
        )
        .map_err(Into::into),
        _ => return reference_use(origins, source),
    })
}

fn reference_use(
    origins: &ForgeOrigins,
    source: QuerySource,
) -> Option<Result<SourceUse, EvidenceError>> {
    Some(match source {
        QuerySource::ReferenceTarget { index, kind } => {
            let field = match kind {
                RecordNameKind::Guid => "guid",
                RecordNameKind::Path => "fileName",
                _ => return None,
            };
            record_site(origins, index, field, SourceUseRole::Selector)
        }
        QuerySource::ReferenceEdge {
            source,
            ordinal,
            target,
        } => site(
            &origins.scope,
            "dataforgeReferenceIndex",
            "incoming",
            vec![u64::from(source), ordinal, u64::from(target)],
            SourceUseRole::Selector,
        )
        .map_err(Into::into),
        QuerySource::ReferenceScan { end } => site(
            &origins.scope,
            "dataforgeReferenceIndex",
            "scannedRecords",
            vec![0, end],
            SourceUseRole::Selector,
        )
        .map_err(Into::into),
        QuerySource::ReferenceGap { record, ordinal } => {
            let mut indices = vec![u64::from(record)];
            let field = ordinal.map_or("unreadableRecord", |ordinal| {
                indices.push(ordinal);
                "unreadableValue"
            });
            site(
                &origins.scope,
                "dataforgeReferenceIndex",
                field,
                indices,
                SourceUseRole::Selector,
            )
            .map_err(Into::into)
        }
        _ => return None,
    })
}
