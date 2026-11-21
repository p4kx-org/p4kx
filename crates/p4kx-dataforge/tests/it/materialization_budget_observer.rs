// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::ControlFlow;
use std::sync::{Arc, Mutex};

use p4kx_dataforge::{
    CanonicalReadObserver, DataForge, DataForgeError, Record, RecordTree, TreeBuilder,
};

use super::fixture::{self, TestResult};

#[derive(Default)]
struct Capture {
    calls: Mutex<Vec<(u32, Result<(), DataForgeError>)>>,
    stop: bool,
}

impl CanonicalReadObserver for Capture {
    fn observe(
        &self,
        record: &Record<'_>,
        outcome: Result<&RecordTree<'_>, &DataForgeError>,
    ) -> ControlFlow<()> {
        let Ok(mut calls) = self.calls.lock() else {
            return ControlFlow::Break(());
        };
        calls.push((record.index(), outcome.map(|_| ()).map_err(Clone::clone)));
        if self.stop {
            ControlFlow::Break(())
        } else {
            ControlFlow::Continue(())
        }
    }
}

#[test]
fn resource_refusal_is_observed_once_without_a_partial_tree_and_next_read_is_unlimited()
-> TestResult {
    let bytes = fixture::array_bytes(3)?;
    fixture::original_array(&bytes, 3)?;
    let forge = DataForge::open(&bytes)?;
    let record = forge.record(0).ok_or("actual observed original owner")?;
    let capture = Arc::new(Capture::default());
    let mut builder = TreeBuilder::with_read_observer(capture.clone());
    let original = DataForgeError::MaterializationBudgetExceeded {
        limit: 3,
        required: 4,
    };
    assert_eq!(
        builder
            .build_with_budget(&record, 3)
            .expect_err("caller allowance is insufficient"),
        original,
        "the actual resource refusal is returned intact"
    );
    let complete = builder.build(&record)?;
    assert_eq!(
        complete.error_count(),
        0,
        "the same Builder returns complete actual source afterward"
    );
    let calls = capture.calls.lock().map_err(|_| "capture lock")?;
    assert_eq!(
        calls.as_slice(),
        [(0, Err(original)), (0, Ok(()))],
        "one actual outcome per read and no synthetic source-tree facts"
    );
    drop(calls);
    Ok(())
}

#[test]
fn caller_observer_stop_keeps_priority_over_the_observed_resource_refusal() -> TestResult {
    let bytes = fixture::array_bytes(3)?;
    fixture::original_array(&bytes, 3)?;
    let forge = DataForge::open(&bytes)?;
    let record = forge.record(0).ok_or("actual original owner")?;
    let capture = Arc::new(Capture {
        stop: true,
        ..Capture::default()
    });
    let mut builder = TreeBuilder::with_read_observer(capture.clone());
    assert!(
        matches!(
            builder.build_with_budget(&record, 3),
            Err(DataForgeError::ObservationStopped)
        ),
        "the caller-owned stop remains distinct and authoritative"
    );
    builder.set_read_observer(None);
    assert_eq!(
        builder.build_with_budget(&record, 4)?.error_count(),
        0,
        "the same Builder remains reusable after observer stop"
    );
    let calls = capture.calls.lock().map_err(|_| "capture lock")?;
    assert_eq!(
        calls.as_slice(),
        [(
            0,
            Err(DataForgeError::MaterializationBudgetExceeded {
                limit: 3,
                required: 4,
            })
        )],
        "the observer receives the actual underlying resource result before choosing to stop"
    );
    drop(calls);
    Ok(())
}
