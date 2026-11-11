// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::ControlFlow;

use p4kx_dataforge::{Record, RecordTree};

use crate::LoadoutError;
use crate::loadout_path::PathRead;
use crate::resolver_observation::Observation;
use crate::spec::EntitySpec;

pub(crate) fn parse_spec(
    record: &Record<'_>,
    tree: &RecordTree<'_>,
    observation: Option<&Observation<'_>>,
) -> Result<EntitySpec, LoadoutError> {
    with_record_paths(record, tree, observation, |paths| {
        EntitySpec::read(tree, paths)
    })
}

pub(crate) fn with_record_paths<T>(
    record: &Record<'_>,
    tree: &RecordTree<'_>,
    observation: Option<&Observation<'_>>,
    mut read: impl FnMut(&PathRead<'_>) -> T,
) -> Result<T, LoadoutError> {
    let Some(observation) = observation else {
        return Ok(read(&PathRead::new(None, None)));
    };
    let mut called = false;
    let mut repeated = false;
    let mut result = None;
    let outcome = observation.record_paths(record, tree, &mut |observer| {
        if called {
            repeated = true;
            return ControlFlow::Break(());
        }
        called = true;
        let paths = PathRead::new(observer, Some(observation));
        let value = read(&paths);
        if paths.stopped() {
            return ControlFlow::Break(());
        }
        result = Some(value);
        ControlFlow::Continue(())
    });
    if outcome.is_break() || repeated || observation.stopped() {
        observation.stop();
        return Err(LoadoutError::ObservationStopped);
    }
    result.map_or_else(
        || {
            observation.stop();
            Err(LoadoutError::ObservationStopped)
        },
        Ok,
    )
}
#[cfg(test)]
#[path = "resolver_path_read_tests.rs"]
mod tests;
