// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::{Mutex, PoisonError};

use rayon::prelude::*;

use crate::query::compile::Plan;
use crate::query::selection_control::{Control, EvaluationError};
use crate::query::source::QuerySource;
use crate::query::{CompiledQuery, QueryRun, SelectionError};
use crate::query::{QueryAdmissionOutcome, QueryRecordAdmission};

impl<'f> CompiledQuery<'f> {
    /// Returns whether Record admission uses only original paths and types.
    #[must_use]
    pub const fn has_header_only_selection(&self) -> bool {
        self.predicate.is_none() && self.referrers.is_none()
    }

    /// Selects Records in path order, retaining failed rows for unreadable Record layouts.
    ///
    /// # Errors
    ///
    /// Returns [`SelectionError`] when a demanded value cannot be read to decide the condition.
    pub fn run(&self) -> Result<QueryRun<'_, 'f>, SelectionError> {
        let proceed = || true;
        self.select(&Control::new(&proceed, None))
            .map(|records| self.resume(records))
    }

    /// Selects Records while `proceed` permits work at Records, path steps and array elements.
    /// Returns `Ok(None)` after any false answer, dropping every partial result.
    ///
    /// # Errors
    ///
    /// Returns [`SelectionError`] when a demanded value cannot be read to decide the condition.
    pub fn run_while(
        &self,
        proceed: impl Fn() -> bool + Sync,
    ) -> Result<Option<QueryRun<'_, 'f>>, SelectionError> {
        self.run_controlled(&Control::new(&proceed, None))
    }

    /// Selects Records and observes the actual inputs read while evaluating their conditions.
    ///
    /// # Errors
    /// Returns the same selection error as the ordinary controlled run.
    pub fn run_observed(
        &self,
        proceed: impl Fn() -> bool + Sync,
        observer: impl Fn(QuerySource) + Sync,
    ) -> Result<Option<QueryRun<'_, 'f>>, SelectionError> {
        self.run_controlled(&Control::new(&proceed, Some(&observer)))
    }

    /// Selects the same Records and observes physical admission and original predicate inputs.
    ///
    /// Reports unreadable filenames excluded by the canonical Record path ordering. Admission
    /// observations may be called in parallel and before a later predicate rejects the scan.
    ///
    /// # Errors
    ///
    /// Returns the same error as the ordinary controlled run.
    pub fn run_observing_records(
        &self,
        proceed: impl Fn() -> bool + Sync,
        observe_operand: impl Fn(QuerySource) + Sync,
        observe_record: impl Fn(QueryRecordAdmission) + Sync,
    ) -> Result<Option<QueryRun<'_, 'f>>, SelectionError> {
        let control =
            Control::new(&proceed, Some(&observe_operand)).observe_admissions(&observe_record);
        self.run_controlled(&control)
    }

    fn run_controlled(
        &self,
        control: &Control<'_>,
    ) -> Result<Option<QueryRun<'_, 'f>>, SelectionError> {
        let records = self.select(control);
        if control.proceed() {
            records.map(|records| Some(self.resume(records)))
        } else {
            Ok(None)
        }
    }

    fn select(&self, control: &Control<'_>) -> Result<Vec<u32>, SelectionError> {
        let failure = Mutex::new(None::<(usize, SelectionError)>);
        let paths = self.forge.path_order();
        if let Some(observe) = control.admission_sink() {
            self.observe_missing_paths(paths, control, observe);
        }
        let records = paths
            .par_iter()
            .copied()
            .enumerate()
            .filter(|&(position, record)| {
                if !control.proceed() {
                    return false;
                }
                let Some(path) = self.forge.path_of(record) else {
                    self.observe_candidate(control, record, QueryAdmissionOutcome::UnreadablePath);
                    return false;
                };
                match self.selects(record, path, control) {
                    Ok(outcome) => {
                        self.observe_candidate(control, record, outcome);
                        outcome.is_admitted()
                    }
                    Err(EvaluationError::Stopped) => false,
                    Err(EvaluationError::Read { comparison, source }) => {
                        self.observe_candidate(
                            control,
                            record,
                            QueryAdmissionOutcome::PredicateReadFailed,
                        );
                        let mut first = failure.lock().unwrap_or_else(PoisonError::into_inner);
                        if first.as_ref().is_none_or(|(at, _)| position < *at) {
                            *first = Some((
                                position,
                                SelectionError::new(record, path, comparison, source),
                            ));
                        }
                        false
                    }
                }
            })
            .map(|(_, record)| record)
            .collect();
        match failure.into_inner().unwrap_or_else(PoisonError::into_inner) {
            Some((_, error)) => Err(error),
            None => Ok(records),
        }
    }

    fn observe_missing_paths(
        &self,
        paths: &[u32],
        control: &Control<'_>,
        observe: &(dyn Fn(QueryRecordAdmission) + Sync),
    ) {
        if paths.len() == self.forge.record_count() {
            return;
        }
        for record in self.forge.records() {
            if !control.proceed() {
                return;
            }
            if record.stored_file_name().is_err() {
                observe(QueryRecordAdmission::observed(
                    self.forge,
                    record.index(),
                    self.pattern.is_some(),
                    QueryAdmissionOutcome::UnreadablePath,
                ));
            }
        }
    }

    fn observe_candidate(
        &self,
        control: &Control<'_>,
        record: u32,
        outcome: QueryAdmissionOutcome,
    ) {
        if let Some(observe) = control.admission_sink() {
            observe(QueryRecordAdmission::observed(
                self.forge,
                record,
                self.pattern.is_some(),
                outcome,
            ));
        }
    }

    fn selects(
        &self,
        record: u32,
        path: &str,
        control: &Control<'_>,
    ) -> Result<QueryAdmissionOutcome, EvaluationError> {
        use QueryAdmissionOutcome::{
            Admitted, NoTypePlan, PathRejected, PredicateRejected, ReferenceRejected,
            RetainedUnreadable,
        };

        let Some(plan) = self.plan_of(record) else {
            return Ok(NoTypePlan);
        };
        if self
            .referrers
            .as_ref()
            .is_some_and(|referrers| referrers.binary_search(&record).is_err())
        {
            return Ok(ReferenceRejected);
        }
        if self
            .pattern
            .as_ref()
            .is_some_and(|pattern| !pattern.matches(path))
        {
            return Ok(PathRejected);
        }
        let Some(predicate) = &self.predicate else {
            return Ok(Admitted);
        };
        match plan {
            Plan::Read { tests, .. } => self
                .instance_of_observed(record, control.observer())
                .map_or(Ok(RetainedUnreadable), |place| {
                    predicate
                        .eval(self.forge, tests, place, control)
                        .map(|value| {
                            if value == Some(true) {
                                Admitted
                            } else {
                                PredicateRejected
                            }
                        })
                }),
            Plan::Broken(_) => Ok(RetainedUnreadable),
        }
    }
}
