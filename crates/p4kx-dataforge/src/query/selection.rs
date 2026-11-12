// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::{Mutex, PoisonError};

use rayon::prelude::*;

use crate::query::compile::Plan;
use crate::query::selection_control::{Control, EvaluationError};
use crate::query::source::QuerySource;
use crate::query::{CompiledQuery, QueryRun, SelectionError};

impl<'f> CompiledQuery<'f> {
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
        let records = self
            .forge
            .path_order()
            .par_iter()
            .copied()
            .enumerate()
            .filter(|&(position, record)| {
                if !control.proceed() {
                    return false;
                }
                let Some(path) = self.forge.path_of(record) else {
                    return false;
                };
                match self.selects(record, path, control) {
                    Ok(holds) => holds,
                    Err(EvaluationError::Stopped) => false,
                    Err(EvaluationError::Read { comparison, source }) => {
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

    fn selects(
        &self,
        record: u32,
        path: &str,
        control: &Control<'_>,
    ) -> Result<bool, EvaluationError> {
        let Some(plan) = self.plan_of(record) else {
            return Ok(false);
        };
        if self
            .referrers
            .as_ref()
            .is_some_and(|referrers| referrers.binary_search(&record).is_err())
            || self
                .pattern
                .as_ref()
                .is_some_and(|pattern| !pattern.matches(path))
        {
            return Ok(false);
        }
        let Some(predicate) = &self.predicate else {
            return Ok(true);
        };
        match plan {
            Plan::Read { tests, .. } => self.instance_of(record).map_or(Ok(true), |place| {
                predicate
                    .eval(self.forge, tests, place, control)
                    .map(|value| value == Some(true))
            }),
            Plan::Broken(_) => Ok(true),
        }
    }
}
