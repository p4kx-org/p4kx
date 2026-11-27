// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::ControlFlow;
use std::sync::Arc;

use p4kx_dataforge::{Record, RecordTree};
use p4kx_extraction::DataForgeOrigin;

use crate::LoadoutError;
use crate::loadout_path::PathRead;
use crate::loadout_xml_record_selector::XmlRecordPaths;
use crate::resolver_observation::Observation;
use crate::spec::EntitySpec;

pub(crate) fn parse_spec<'source>(
    record: &Record<'source>,
    tree: &RecordTree<'source>,
    observation: Option<&Observation<'_>>,
    forge_origin: Option<&Arc<DataForgeOrigin>>,
) -> Result<EntitySpec, LoadoutError> {
    let forge_origin = forge_origin.filter(|_| tree.is_from(record));
    with_record_paths(record, tree, observation, |paths| {
        EntitySpec::read(tree, paths, forge_origin)
    })
}

pub(crate) fn with_record_paths<'source, T>(
    record: &Record<'source>,
    tree: &RecordTree<'source>,
    observation: Option<&Observation<'_>>,
    mut read: impl FnMut(&PathRead<'_>) -> T,
) -> Result<T, LoadoutError> {
    let Some(observation) = observation else {
        return Ok(read(&PathRead::new(None, None)));
    };
    let xml_record_paths = (observation.observes_xml_record_selectors() && tree.is_from(record))
        .then(XmlRecordPaths::default);
    let mut called = false;
    let mut repeated = false;
    let mut result = None;
    let outcome = observation.record_paths(record, tree, &mut |observer| {
        if called {
            repeated = true;
            return ControlFlow::Break(());
        }
        called = true;
        let paths = PathRead::new(observer, Some(observation))
            .with_xml_record_paths(xml_record_paths.as_ref());
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
    let Some(result) = result else {
        observation.stop();
        return Err(LoadoutError::ObservationStopped);
    };
    if let Some(paths) = xml_record_paths
        && paths.finish(record, tree, observation).is_break()
    {
        return Err(LoadoutError::ObservationStopped);
    }
    Ok(result)
}
#[cfg(test)]
#[path = "resolver_path_read_tests.rs"]
mod tests;
