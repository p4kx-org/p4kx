// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::ControlFlow;

use p4kx_assembly::{LoadoutReadEvent, LoadoutReadObserver};

use super::{Arc, Capture, ExportLedger};

#[test]
fn poisoned_capture_stops_observation_and_refuses_final_evidence() {
    let capture = Arc::new(Capture::new(&ExportLedger::default(), None));
    assert!(
        crate::export_evidence_lock::poison(&capture.state),
        "the synthetic worker actually poisons the evidence lock"
    );
    assert_eq!(
        capture.observe(LoadoutReadEvent::InventoryIncomplete),
        ControlFlow::Break(()),
        "a poisoned capture stops source callbacks without inspecting its state"
    );
    assert!(
        matches!(
            capture.finish(),
            Err(crate::CatalogError::Accounting(
                crate::ExportAccountingError::PoisonedEvidence
            ))
        ),
        "mandatory finalization reports the typed poison failure"
    );
    assert!(
        capture.seed_proofs(&ExportLedger::default()).is_err(),
        "cached proof forwarding cannot silently accept poisoned evidence"
    );
}
