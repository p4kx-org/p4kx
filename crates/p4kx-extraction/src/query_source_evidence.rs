// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Addresses original DataForge inputs reported by the Query reader.

use p4kx_dataforge::query::{QueryByteUse, QuerySource, RecordNameKind, TraversalInput};

use crate::{SourceEvidenceError, SourceLocation, SourceScope, SourceUse, SourceUseRole};

/// An original Query input coordinate and its observed contribution role.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct QuerySourceSite {
    /// The original coordinate reported by the Query reader.
    pub location: SourceLocation,
    /// How the reader used that coordinate.
    pub role: SourceUseRole,
}

impl QuerySourceSite {
    /// Resolves one Query observation without registering a source use.
    #[must_use]
    pub fn of(source: QuerySource, value_role: SourceUseRole) -> Option<Self> {
        match source {
            QuerySource::Bytes { start, end, kind } => bytes(start, end, kind, value_role),
            QuerySource::Record { index } => {
                Some(Self::record(index, "instance", SourceUseRole::Selector))
            }
            QuerySource::EnumCount => Some(Self::header("enumCount", SourceUseRole::Selector)),
            QuerySource::TextLength => Some(Self::header("textLength", SourceUseRole::Selector)),
            QuerySource::RecordType { index } => {
                Some(Self::record(index, "sourceType", SourceUseRole::Selector))
            }
            QuerySource::ArrayElement {
                header_start,
                index,
            } => Some(Self::selector(
                "dataforgeArrayElement",
                "selection",
                vec![header_start, u64::from(index)],
            )),
            QuerySource::Instance {
                structure,
                instance,
            } => Some(Self::selector(
                "dataforgeInstance",
                "selection",
                vec![u64::from(structure), u64::from(instance)],
            )),
            QuerySource::AncestorParent { index, .. } => {
                Some(Self::structure(index, "parent", SourceUseRole::Selector))
            }
            QuerySource::Structure { index } => Some(Self::selector(
                "dataforgeStructDefinition",
                "sourceType",
                vec![u64::from(index)],
            )),
            _ => reference(source),
        }
    }

    /// Addresses one original Record header field read by the caller.
    #[must_use]
    pub fn record(index: u32, field: &str, role: SourceUseRole) -> Self {
        Self {
            location: SourceLocation::DataForge {
                record_index: index,
                field: field.into(),
            },
            role,
        }
    }

    /// Registers the coordinate against the DataForge scope that supplied the observation.
    ///
    /// # Errors
    ///
    /// Returns the error of registering this use in the source inventory.
    pub fn bind(self, scope: &SourceScope) -> Result<SourceUse, SourceEvidenceError> {
        scope.at(self.location, self.role)
    }

    pub(crate) fn header(field: &str, role: SourceUseRole) -> Self {
        Self::subresource("dataforgeHeader", field, Vec::new(), role)
    }

    pub(crate) fn structure(index: usize, field: &str, role: SourceUseRole) -> Self {
        Self::subresource("dataforgeStructDefinition", field, vec![index as u64], role)
    }

    fn selector(namespace: &str, field: &str, indices: Vec<u64>) -> Self {
        Self::subresource(namespace, field, indices, SourceUseRole::Selector)
    }

    fn subresource(namespace: &str, field: &str, indices: Vec<u64>, role: SourceUseRole) -> Self {
        Self {
            location: SourceLocation::Subresource {
                namespace: namespace.into(),
                indices,
                field: field.into(),
            },
            role,
        }
    }
}

fn bytes(
    start: u64,
    end: u64,
    kind: QueryByteUse,
    value_role: SourceUseRole,
) -> Option<QuerySourceSite> {
    let (label, role) = match kind {
        QueryByteUse::Value => ("slot", value_role),
        QueryByteUse::AttemptedValue => ("slot", SourceUseRole::Attempted),
        QueryByteUse::AttemptedTraversal(input) => (
            match input {
                TraversalInput::Value => "slot",
                TraversalInput::Pointer => "pointer",
                TraversalInput::Reference => "reference",
                TraversalInput::WeakLink => "weakLink",
                TraversalInput::Array => "arrayHeader",
                _ => return None,
            },
            SourceUseRole::Attempted,
        ),
        QueryByteUse::Text => ("text", value_role),
        QueryByteUse::TextTerminator => ("textTerminator", SourceUseRole::Selector),
        QueryByteUse::AttemptedText => ("text", SourceUseRole::Attempted),
        QueryByteUse::PoolBound => ("poolCount", SourceUseRole::Selector),
        QueryByteUse::Array => ("arrayHeader", SourceUseRole::Selector),
        QueryByteUse::Pointer => ("pointer", SourceUseRole::Selector),
        QueryByteUse::Reference => ("reference", SourceUseRole::Selector),
        QueryByteUse::DanglingReference => ("reference", SourceUseRole::Attempted),
        QueryByteUse::WeakLink => ("weakLink", SourceUseRole::Selector),
        QueryByteUse::Unresolved => ("unresolvedLink", SourceUseRole::Selector),
        _ => return None,
    };
    Some(QuerySourceSite::subresource(
        "dataforgeDecodedBytes",
        label,
        vec![start, end],
        role,
    ))
}

fn reference(source: QuerySource) -> Option<QuerySourceSite> {
    Some(match source {
        QuerySource::RecordLookup { kind, end } => QuerySourceSite::selector(
            "dataforgeRecordHeaderRange",
            record_column(kind)?,
            vec![0, end],
        ),
        QuerySource::ReferenceTarget { index, kind } => {
            QuerySourceSite::record(index, record_column(kind)?, SourceUseRole::Selector)
        }
        QuerySource::ReferenceEdge {
            source,
            ordinal,
            target,
        } => QuerySourceSite::selector(
            "dataforgeReferenceIndex",
            "incoming",
            vec![u64::from(source), ordinal, u64::from(target)],
        ),
        QuerySource::ReferenceScan { end } => {
            QuerySourceSite::selector("dataforgeReferenceIndex", "scannedRecords", vec![0, end])
        }
        QuerySource::ReferenceGap { record, ordinal } => {
            let mut indices = vec![u64::from(record)];
            let field = ordinal.map_or("unreadableRecord", |ordinal| {
                indices.push(ordinal);
                "unreadableValue"
            });
            QuerySourceSite::selector("dataforgeReferenceIndex", field, indices)
        }
        _ => return None,
    })
}

const fn record_column(kind: RecordNameKind) -> Option<&'static str> {
    Some(match kind {
        RecordNameKind::Guid => "guid",
        RecordNameKind::Path => "fileName",
        _ => return None,
    })
}

#[cfg(test)]
#[path = "query_source_evidence_tests.rs"]
mod tests;
