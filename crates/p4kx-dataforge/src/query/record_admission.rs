// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use crate::{DataForge, RecordOrderingName, RecordPathObservation};

/// The first actual decision about an original query candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum QueryAdmissionOutcome {
    /// A physical header was omitted because its stored filename could not be read.
    UnreadablePath,
    /// There was no matching compiled type plan for the original Record.
    NoTypePlan,
    /// The compiled Reference filter rejects the Record.
    ReferenceRejected,
    /// The original file path failed the explicit path filter.
    PathRejected,
    /// The predicate was evaluated and did not admit the instance.
    PredicateRejected,
    /// The predicate needed a value that could not be read.
    PredicateReadFailed,
    /// The selected Record layout or instance is retained as unreadable by the original query.
    RetainedUnreadable,
    /// The Record was selected by the ordinary query.
    Admitted,
}

impl QueryAdmissionOutcome {
    pub(super) const fn is_admitted(self) -> bool {
        matches!(self, Self::Admitted | Self::RetainedUnreadable)
    }
}

/// The original header and observed decision for one query candidate.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct QueryRecordAdmission {
    /// The exact physical Record table position.
    pub record_index: u32,
    /// The selected type header operand, if admission advanced through type planning.
    pub structure_index: Option<u32>,
    /// The original NUL-terminated filename bytes, when readable.
    pub file_name: Option<Range<u64>>,
    /// The original NUL-terminated ordering name, when readable.
    pub record_name: Option<Range<u64>>,
    /// The original ordering-name availability.
    pub name_state: RecordOrderingName,
    /// Whether the existing query actually tested its explicit AT path criterion.
    pub path_filter_checked: bool,
    /// The ordinary candidate decision.
    pub outcome: QueryAdmissionOutcome,
}

impl QueryRecordAdmission {
    pub(super) fn observed(
        forge: &DataForge<'_>,
        record_index: u32,
        has_path_filter: bool,
        outcome: QueryAdmissionOutcome,
    ) -> Self {
        let record = forge.record(record_index);
        let path_filter_checked = has_path_filter
            && !matches!(
                outcome,
                QueryAdmissionOutcome::UnreadablePath
                    | QueryAdmissionOutcome::NoTypePlan
                    | QueryAdmissionOutcome::ReferenceRejected,
            );
        let structure_index = if outcome == QueryAdmissionOutcome::UnreadablePath {
            None
        } else {
            record.map(|record| record.def.struct_index)
        };
        let header = record.map(|record| RecordPathObservation::read(&record));
        let (file_name, record_name, name_state) = header
            .map_or((None, None, RecordOrderingName::NotConsulted), |header| {
                (header.file_name, header.record_name, header.name_state)
            });
        Self {
            record_index,
            structure_index,
            file_name,
            record_name,
            name_state,
            path_filter_checked,
            outcome,
        }
    }
}
