// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::{FieldStatisticsGap, FieldStatisticsInput};
use crate::DataType;

/// Whether a single field metric is covered by the actual admitted walk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum FieldStatisticsCoverageState {
    /// The admitted walk leaves no unresolved input affecting this metric.
    Covered,
    /// An unresolved input may affect the result.
    Incomplete,
}

impl FieldStatisticsCoverageState {
    const fn from_coverage(covered: bool) -> Self {
        if covered {
            Self::Covered
        } else {
            Self::Incomplete
        }
    }

    /// Returns whether the admitted walk covers this metric.
    #[must_use]
    pub const fn is_covered(self) -> bool {
        matches!(self, Self::Covered)
    }
}

/// Which metric operands were resolved within the scanner's admitted Record scope.
///
/// The aggregate's source scope independently describes gaps in Record selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct FieldStatisticsMetricCoverage {
    /// All possible occurrences and their storage kinds were enumerated at this path.
    pub structure: FieldStatisticsCoverageState,
    /// No unresolved occurrence could change the count of empty values.
    pub empty: FieldStatisticsCoverageState,
    /// No unresolved occurrence could change the reported distinct count or cap state.
    pub distinct: FieldStatisticsCoverageState,
    /// No unresolved numeric occurrence could change the extrema.
    pub extrema: FieldStatisticsCoverageState,
}

/// Gaps observed in the same structural path during the statistics scan.
#[derive(Debug, Clone, Copy)]
pub(super) struct Gaps {
    here: bool,
    descendants: bool,
    values: ValueGaps,
}

#[derive(Debug, Clone, Copy)]
struct ValueGaps {
    empty: bool,
    distinct: bool,
    extrema: bool,
}

impl Gaps {
    pub(super) const fn new() -> Self {
        Self {
            here: false,
            descendants: false,
            values: ValueGaps {
                empty: false,
                distinct: false,
                extrema: false,
            },
        }
    }

    pub(super) const fn root_unreadable(&mut self) {
        self.descendants = true;
    }

    pub(super) fn observe(
        &mut self,
        reason: FieldStatisticsGap,
        input: Option<&FieldStatisticsInput>,
    ) {
        let data_type = input.and_then(storage);
        match reason {
            FieldStatisticsGap::InvalidArrayExtent => {
                self.here = true;
                self.descendants |= data_type
                    .is_none_or(|kind| matches!(kind, DataType::Class | DataType::StrongPointer));
            }
            FieldStatisticsGap::MissingInstance => {
                self.here |= data_type != Some(DataType::StrongPointer);
                self.descendants = true;
            }
            FieldStatisticsGap::UnreadableLayout => {
                self.descendants = true;
            }
            FieldStatisticsGap::UnreadableValue => self.bad_value(data_type),
            FieldStatisticsGap::MissingRecord | FieldStatisticsGap::RevisitedInstance => {}
        }
    }

    const fn bad_value(&mut self, data_type: Option<DataType>) {
        let Some(data_type) = data_type else {
            self.values.empty = true;
            self.values.distinct = true;
            self.values.extrema = true;
            return;
        };
        self.values.empty |= matches!(
            data_type,
            DataType::String
                | DataType::Locale
                | DataType::Enum
                | DataType::Guid
                | DataType::Reference
        );
        self.values.distinct |= !matches!(
            data_type,
            DataType::Class | DataType::StrongPointer | DataType::WeakPointer
        );
        self.values.extrema |= matches!(
            data_type,
            DataType::Int8
                | DataType::Int16
                | DataType::Int32
                | DataType::Int64
                | DataType::UInt8
                | DataType::UInt16
                | DataType::UInt32
                | DataType::UInt64
                | DataType::Single
                | DataType::Double
        );
    }

    pub(super) const fn merge(&mut self, other: Self) {
        self.here |= other.here;
        self.descendants |= other.descendants;
        self.values.empty |= other.values.empty;
        self.values.distinct |= other.values.distinct;
        self.values.extrema |= other.values.extrema;
    }

    pub(super) const fn covers_descendants(self, inherited_gap: bool) -> bool {
        inherited_gap || self.descendants
    }

    pub(super) const fn coverage(
        self,
        inherited_gap: bool,
        capped: bool,
    ) -> FieldStatisticsMetricCoverage {
        let structure_complete = !inherited_gap && !self.here;
        FieldStatisticsMetricCoverage {
            structure: FieldStatisticsCoverageState::from_coverage(structure_complete),
            empty: FieldStatisticsCoverageState::from_coverage(
                structure_complete && !self.values.empty,
            ),
            distinct: FieldStatisticsCoverageState::from_coverage(
                structure_complete && (!self.values.distinct || capped),
            ),
            extrema: FieldStatisticsCoverageState::from_coverage(
                structure_complete && !self.values.extrema,
            ),
        }
    }
}

const fn storage(input: &FieldStatisticsInput) -> Option<DataType> {
    match input {
        FieldStatisticsInput::Slot(slot) => Some(slot.kind.data_type()),
        FieldStatisticsInput::ArrayElement { header, .. } => Some(header.kind.data_type()),
        FieldStatisticsInput::Record(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{FieldStatisticsGap, Gaps};

    #[test]
    fn a_capped_result_ignores_unknown_values_but_not_an_unresolved_scope() {
        let mut gaps = Gaps::new();
        gaps.observe(FieldStatisticsGap::UnreadableValue, None);
        assert!(
            !gaps.coverage(false, false).distinct.is_covered(),
            "an unreadable value can add a distinct fingerprint"
        );
        assert!(
            gaps.coverage(false, true).distinct.is_covered(),
            "the existing positive capped cohort already fixes the reported distinct count"
        );
        assert!(
            !gaps.coverage(true, true).distinct.is_covered(),
            "an unresolved enclosing shape can change the admissible original occurrences"
        );
    }
}
