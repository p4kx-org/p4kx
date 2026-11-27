// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use p4kx_dataforge::{
    FieldStatistics, FieldStatisticsGap, FieldStatisticsInput, FieldStatisticsObservation,
    FieldStatisticsUse,
};

use crate::{FieldOrigin, SharedFieldOrigin, SourceUseRef, SourceUses};

use super::FieldStatisticsReportError;
use super::rows::Rows;

/// The source-backed summary values reported by the field-statistics command.
#[derive(Debug)]
#[non_exhaustive]
pub struct FieldStatisticsCounterProof {
    /// Original Record selection and examined root positions.
    pub records: FieldOrigin,
    /// Original root visits used by the unreadable Record counter.
    pub unreadable_records: FieldOrigin,
    /// Original scanned operands used by the unreadable/skipped value counter.
    pub unreadable_values: FieldOrigin,
    /// Original path shape needed to count the fields after a display filter.
    pub field_count: FieldOrigin,
    /// Failed original value-read attempts, including paths with no emitted row.
    pub value_attempts: Vec<SourceUseRef>,
}

#[derive(Default)]
pub(super) struct ScanAccounting {
    roots: u64,
    unreadable_roots: u64,
    skipped_values: u64,
    shape_unavailable: bool,
}

impl ScanAccounting {
    pub(super) const fn observe(&mut self, event: &FieldStatisticsObservation<'_>) {
        match (event.input(), event.usage()) {
            (FieldStatisticsInput::Record(_), FieldStatisticsUse::Record { .. }) => {
                self.roots += 1;
            }
            (FieldStatisticsInput::Record(_), FieldStatisticsUse::Skipped(_)) => {
                self.roots += 1;
                self.unreadable_roots += 1;
            }
            (_, FieldStatisticsUse::Skipped(gap)) => {
                self.skipped_values += 1;
                self.shape_unavailable |= matches!(
                    gap,
                    FieldStatisticsGap::MissingInstance
                        | FieldStatisticsGap::UnreadableLayout
                        | FieldStatisticsGap::InvalidArrayExtent
                );
            }
            _ => {}
        }
    }

    pub(super) fn bind(
        &self,
        sources: &SourceUses,
        statistics: &FieldStatistics,
        selection: &SharedFieldOrigin,
        rows: &Rows,
    ) -> Result<FieldStatisticsCounterProof, FieldStatisticsReportError> {
        if (self.roots, self.unreadable_roots, self.skipped_values)
            != (
                statistics.records(),
                statistics.unreadable_records(),
                statistics.unreadable_values(),
            )
        {
            return Err(FieldStatisticsReportError::UnboundCounters);
        }
        let records = sources.origin_with_shared_uses(selection, [], true)?;
        let unreadable_values =
            sources.origin_with_shared_uses(selection, rows.checked_values(), true)?;
        let field_count = sources.origin_with_shared_uses(
            selection,
            rows.shape_uses(),
            self.unreadable_roots == 0 && !self.shape_unavailable,
        )?;
        let value_attempts = rows
            .value_attempts()
            .map(|usage| usage.reference().clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        Ok(FieldStatisticsCounterProof {
            unreadable_records: records.clone(),
            records,
            unreadable_values,
            field_count,
            value_attempts,
        })
    }
}
