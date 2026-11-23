// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use super::seen::Seen;
use super::{FieldKind, FieldStatisticsScalarRead, Number};
use crate::LayoutSource;

/// The way the field-statistics scanner used an original input.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum FieldStatisticsUse {
    /// An admitted original declaration or array element was consulted.
    Consulted,
    /// An actual original declaration consulted while exhausting an admitted layout.
    Layout(LayoutSource),
    /// A selected Record reached a readable root instance and layout.
    Record {
        /// The original Struct table index of the Record instance.
        structure: u32,
        /// The position of the Record instance in that Struct's instance block.
        instance: u32,
    },
    /// An admitted physical child instance or inline extent was selected by the original input.
    #[non_exhaustive]
    Instance {
        /// The physical Struct table index for the accepted layout or instance.
        structure: u32,
        /// The physical instance position in that Struct block.
        instance: u32,
        /// The consulted child extent when the decoded bytes have an original position.
        bytes: Option<Range<u64>>,
        /// The original concrete type name contributing a new path segment, when used.
        path_name: Option<Range<u64>>,
    },
    /// The scanner counted this occurrence, even if its value was unreadable.
    #[non_exhaustive]
    Counted {
        /// The same storage kind counted by the statistics reducer.
        kind: FieldKind,
        /// The outcome of the actual value read.
        value: FieldStatisticsValue,
        /// Original decoder inputs used by this occurrence.
        scalar_read: FieldStatisticsScalarRead,
    },
    /// A checked position contributed to the unreadable or skipped count.
    Skipped(FieldStatisticsGap),
}

/// The metric inputs retained for one counted occurrence.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum FieldStatisticsValue {
    /// An authored empty value, with the text span if it came from a text table.
    Empty(Option<Range<u64>>),
    /// A structural occurrence with no scalar or distinct numeric value.
    Structural,
    /// A nonempty readable value considered for distinct counting.
    #[non_exhaustive]
    Nonempty {
        /// The same hash operand used by the existing distinct-value counter.
        fingerprint: u64,
        /// The decision in this scan partition's distinct-value reducer.
        distinct: FieldStatisticsDistinctUse,
        /// The input to numeric comparison, retaining the original numeric width when applicable.
        number: Option<Number>,
        /// The decoded DCB bytes of the scalar text, when consulted and available.
        text: Option<Range<u64>>,
    },
    /// A field whose occurrence was counted but whose value could not be decoded.
    Unreadable,
}

impl FieldStatisticsValue {
    pub(super) const fn from_read(
        seen: Seen,
        empty: bool,
        text: Option<Range<u64>>,
        distinct: FieldStatisticsDistinctUse,
    ) -> Self {
        match seen {
            Seen::Unreadable => Self::Unreadable,
            Seen::Empty if empty => Self::Empty(text),
            Seen::Empty => Self::Structural,
            Seen::Value {
                number,
                fingerprint,
            } => Self::Nonempty {
                fingerprint,
                distinct,
                number,
                text,
            },
        }
    }
}

/// The actual use of a fingerprint in a scan partition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum FieldStatisticsDistinctUse {
    /// No fingerprint is admitted.
    NoCandidate,
    /// The fingerprint was compared and already present.
    ComparedDuplicate,
    /// The fingerprint was compared and retained.
    Admitted,
    /// This additional fingerprint established that the distinct count is capped.
    ReachedCap,
    /// This partition had already established its capped state.
    AfterCap,
}

/// Why the scanner accounted for a skipped or unreadable input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum FieldStatisticsGap {
    /// No Record header exists for the requested index.
    MissingRecord,
    /// A recorded instance position did not resolve to stored instance bytes.
    MissingInstance,
    /// Layout validation failed before the selected structure could be walked.
    UnreadableLayout,
    /// The authored array header addresses elements outside the instance or value pool.
    InvalidArrayExtent,
    /// A physical instance or inline route was already examined in the Record.
    RevisitedInstance,
    /// The selected value could not be read.
    UnreadableValue,
}
