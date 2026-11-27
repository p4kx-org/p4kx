// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::ControlFlow;
use std::sync::Arc;

use p4kx_dataforge::{DataForge, Record, RecordTree};
use p4kx_testkit::{DataForgeBuilder, TestEntity};

use super as record_read;
use crate::{
    Entities, LoadoutError, LoadoutPathObserver, LoadoutReadEvent, LoadoutReadObserver,
    LoadoutResolver,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

enum Invocation {
    Zero,
    Once,
    Repeated,
}

struct Observer(Invocation);

impl LoadoutReadObserver for Observer {
    fn observe(&self, _event: LoadoutReadEvent<'_, '_>) -> ControlFlow<()> {
        ControlFlow::Continue(())
    }

    fn with_record_paths(
        &self,
        _record: &Record<'_>,
        _tree: &RecordTree<'_>,
        parse: &mut dyn FnMut(Option<&dyn LoadoutPathObserver>) -> ControlFlow<()>,
    ) -> ControlFlow<()> {
        match self.0 {
            Invocation::Zero => ControlFlow::Continue(()),
            Invocation::Once => parse(None),
            Invocation::Repeated => {
                let _first = parse(None);
                let _repeated = parse(None);
                ControlFlow::Continue(())
            }
        }
    }
}

#[test]
fn an_observer_cannot_skip_or_repeat_the_single_actual_projection() -> TestResult {
    let bytes = p4kx_testkit::entity_schema(DataForgeBuilder::new())
        .record(TestEntity::new(1, "Root", "synthetic/root.xml").record())
        .build();
    let forge = DataForge::open(&bytes)?;
    let record = forge.record(0).ok_or("the actual Record is absent")?;
    let tree = record.tree()?;
    for (mode, expected_calls, valid) in [
        (Invocation::Zero, 0, false),
        (Invocation::Once, 1, true),
        (Invocation::Repeated, 1, false),
    ] {
        let observer = Arc::new(Observer(mode));
        let resolver =
            LoadoutResolver::with_observed_entities(Arc::new(Entities::new(&forge)), observer);
        let mut calls = 0;
        let result =
            record_read::with_record_paths(&record, &tree, resolver.observation.as_deref(), |_| {
                calls += 1;
                calls
            });
        assert_eq!(
            calls, expected_calls,
            "the guard permits at most one real projection"
        );
        assert_eq!(
            result.is_ok(),
            valid,
            "zero and repeated callback execution are rejected"
        );
        if !valid {
            assert!(
                matches!(result, Err(LoadoutError::ObservationStopped)),
                "an invalid callback cannot yield a projected spec"
            );
            assert!(
                resolver.observation_stopped(),
                "an invalid callback stops subsequent reads"
            );
        }
    }
    Ok(())
}
