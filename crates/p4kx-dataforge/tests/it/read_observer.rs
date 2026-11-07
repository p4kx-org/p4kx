// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Optional canonical-read observation preserves physical identity and unobserved decoding.

use std::ops::ControlFlow;
use std::sync::{Arc, Mutex};

use p4kx_dataforge::{
    CanonicalReadObserver, DataForge, DataForgeError, Record, RecordTree, TreeBuilder,
};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[derive(Default)]
struct Capture {
    calls: Mutex<Vec<(u32, bool)>>,
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
        calls.push((record.index(), outcome.is_ok()));
        if self.stop {
            ControlFlow::Break(())
        } else {
            ControlFlow::Continue(())
        }
    }
}

fn database(version: u32, malformed: bool) -> Vec<u8> {
    let structure = TestStruct::new("Source").field("value", TestType::Int32);
    DataForgeBuilder::new()
        .version(version)
        .structure(if malformed {
            structure.declared_size(6)
        } else {
            structure
        })
        .record(
            TestRecord::new(1, "Source", "Source.First", "source/first.xml")
                .value("value", TestValue::Int32(7)),
        )
        .record(
            TestRecord::new(1, "Source", "Source.Second", "source/second.xml")
                .value("value", TestValue::Int32(9)),
        )
        .build()
}

#[test]
fn actual_decode_observation_preserves_same_guid_at_distinct_physical_indices() -> TestResult {
    for version in [6, 8] {
        let bytes = database(version, false);
        let forge = DataForge::open(&bytes)?;
        let capture = Arc::new(Capture::default());
        let mut observed = TreeBuilder::with_read_observer(capture.clone());
        let mut ordinary = TreeBuilder::new();
        for record in forge.records() {
            let tree = observed.build(&record)?;
            let plain = ordinary.build(&record)?;
            assert_eq!(
                tree, plain,
                "adding an observer does not alter canonical content"
            );
        }
        assert_eq!(
            *capture.calls.lock().map_err(|_| "capture lock")?,
            [(0, true), (1, true)],
            "physical source identity survives duplicate GUIDs"
        );
    }
    Ok(())
}

#[test]
fn actual_decode_errors_are_observed_once_and_returned_unchanged() -> TestResult {
    let bytes = database(8, true);
    let forge = DataForge::open(&bytes)?;
    let record = forge.record(0).ok_or("source")?;
    let capture = Arc::new(Capture::default());
    let mut trees = TreeBuilder::with_read_observer(capture.clone());
    assert!(
        matches!(
            trees.build(&record),
            Err(DataForgeError::StructLayout { .. })
        ),
        "the original invalid layout remains the returned source error"
    );
    assert_eq!(
        *capture.calls.lock().map_err(|_| "capture lock")?,
        [(0, false)],
        "one decode result produces one observation"
    );
    Ok(())
}

#[test]
fn observer_stop_is_distinct_from_game_data_failure_and_can_be_removed() -> TestResult {
    let bytes = database(8, false);
    let forge = DataForge::open(&bytes)?;
    let record = forge.record(0).ok_or("source")?;
    let capture = Arc::new(Capture {
        stop: true,
        ..Capture::default()
    });
    let mut trees = TreeBuilder::with_read_observer(capture.clone());
    assert!(
        matches!(
            trees.build(&record),
            Err(DataForgeError::ObservationStopped)
        ),
        "a caller stop does not claim malformed game bytes"
    );
    trees.set_read_observer(None);
    assert!(
        trees.build(&record).is_ok(),
        "removing the optional observer restores ordinary decoding on the same builder"
    );
    assert_eq!(
        capture.calls.lock().map_err(|_| "capture lock")?.len(),
        1,
        "unobserved reads produce no callback"
    );
    Ok(())
}
