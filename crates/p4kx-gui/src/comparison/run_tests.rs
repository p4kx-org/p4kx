// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, mpsc};

use p4kx_comparison::CancelToken;

use crate::comparison::{ComparisonStage, ComparisonStatus, ComparisonTab, State};
use crate::discovery_test_support::{self, TestResult};

use super::{Completion, Outcome, Run};

#[test]
fn comparison_poll_retains_open_reports_beside_a_failed_background_outcome() -> TestResult {
    let (sender, result) = mpsc::channel();
    let completion = Completion {
        outcome: Outcome::Failed(Some("primary comparison sentinel".to_owned())),
        issues: discovery_test_support::issues()?,
    };
    assert!(
        sender.send(completion).is_ok(),
        "the production comparison receives the measured completion"
    );
    let alive = Arc::new(());
    let run = Run {
        cancel: CancelToken::new(),
        alive: Arc::downgrade(&alive),
        stage: Arc::new(Mutex::new(ComparisonStage::Opening)),
        key_asked: Arc::new(AtomicBool::new(false)),
        result,
    };
    let mut tab = ComparisonTab {
        state: State::Running(run),
        ..ComparisonTab::default()
    };
    assert!(
        tab.poll(),
        "the actual controller adopts the worker completion"
    );
    assert!(
        matches!(
            tab.status(),
            ComparisonStatus::Failed(Some("primary comparison sentinel"))
        ),
        "the primary comparison reason remains unchanged"
    );
    assert!(
        !tab.discovery_issues().is_empty(),
        "the model exposes the typed open report to the banner"
    );
    Ok(())
}
