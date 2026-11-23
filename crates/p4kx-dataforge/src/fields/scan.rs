// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::atomic::{AtomicBool, Ordering};

use rayon::prelude::*;

use super::finalize;
use super::layouts::SharedLayouts;
use super::observation::{FieldStatisticsObservation, Observe};
use super::part::Part;
use super::{FieldStatistics, FieldStatisticsError, FieldStatisticsReduction, min_part_len};
use crate::{DataForge, RecordOrderingName, RecordPathObservation};

impl FieldStatistics {
    /// Computes statistics while reporting original slots and admitted array elements.
    ///
    /// Observations may arrive in parallel and before a later read failure rejects the scan.
    /// Observe the operation's outcome before publishing evidence derived from it.
    ///
    /// # Errors
    ///
    /// Returns the same errors as `of_records`.
    pub fn of_records_observed<'source>(
        forge: &'source DataForge<'source>,
        records: &[u32],
        observe: impl Fn(FieldStatisticsObservation<'_>) + Sync,
    ) -> Result<Self, FieldStatisticsError> {
        Ok(read(
            forge,
            records,
            || true,
            Some(&observe),
            false,
            finalize::ordinary,
        )?
        .unwrap_or_default())
    }

    /// Reads the same admitted roots as the ordinary all-Record statistics scan.
    ///
    /// Reports the physical header census separately from metric and traversal observations.
    ///
    /// # Errors
    ///
    /// Returns an error when the selected fields cannot be scanned.
    pub fn of_all_with_reduction<'source>(
        forge: &'source DataForge<'source>,
        mut observe_path: impl FnMut(RecordPathObservation),
        observe_value: impl Fn(FieldStatisticsObservation<'_>) + Sync,
    ) -> Result<FieldStatisticsReduction<'source>, FieldStatisticsError> {
        let mut unreadable_paths = 0;
        let mut unreadable_names = 0;
        let records = forge.path_order_observed(|entry| {
            if !entry.indexed {
                unreadable_paths += 1;
            } else if entry.name_state == RecordOrderingName::Unreadable {
                unreadable_names += 1;
            }
            observe_path(entry);
        });
        let mut reduction = Self::of_records_with_reduction(forge, records, observe_value)?;
        reduction.observe_all_paths(unreadable_paths, unreadable_names);
        Ok(reduction)
    }

    /// Computes the unchanged aggregate with original representatives of its merged comparisons.
    ///
    /// The observer receives the individual read operations. The returned reduction binds the final
    /// output ordinals to exact structural paths and selected metric operands.
    ///
    /// # Errors
    ///
    /// Returns the same errors as `of_records_observed`.
    pub fn of_records_with_reduction<'source>(
        forge: &'source DataForge<'source>,
        records: &[u32],
        observe: impl Fn(FieldStatisticsObservation<'_>) + Sync,
    ) -> Result<FieldStatisticsReduction<'source>, FieldStatisticsError> {
        let result = read(
            forge,
            records,
            || true,
            Some(&observe),
            true,
            finalize::with_reduction,
        )?;
        Ok(result.unwrap_or_else(FieldStatisticsReduction::empty))
    }
}

pub(super) fn read<'source, Output>(
    forge: &'source DataForge<'source>,
    records: &[u32],
    proceed: impl Fn() -> bool + Sync,
    observer: Option<Observe<'_>>,
    capture_comparisons: bool,
    finish: impl FnOnce(Part<'_, 'source>) -> Output,
) -> Result<Option<Output>, FieldStatisticsError> {
    let stopped = AtomicBool::new(false);
    let layouts = SharedLayouts::new(forge);
    let part = records
        .par_iter()
        .with_min_len(min_part_len(records.len(), rayon::current_num_threads()))
        .fold(
            || Ok(Part::new(&layouts, observer, capture_comparisons)),
            |part: Result<Part<'_, 'source>, FieldStatisticsError>, &record| {
                let mut part = part?;
                if stopped.load(Ordering::Relaxed) {
                    return Ok(part);
                }
                if !proceed() {
                    stopped.store(true, Ordering::Relaxed);
                    return Ok(part);
                }
                part.add(forge, record)?;
                Ok(part)
            },
        )
        .reduce(
            || Ok(Part::new(&layouts, observer, capture_comparisons)),
            |left, right| left.and_then(|left| left.merge(right?)),
        )?;
    Ok((!stopped.load(Ordering::Relaxed)).then(|| finish(part)))
}
