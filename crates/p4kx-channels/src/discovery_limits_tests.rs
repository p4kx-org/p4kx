// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::{DiscoveryBudget, DiscoveryLimit, DiscoveryLimits};

#[test]
fn payload_admission_accepts_the_exact_limit_and_rejects_an_overflowing_next_byte() {
    let limits = DiscoveryLimits {
        fact_bytes: usize::MAX,
        replayed_bytes: usize::MAX,
        ..DiscoveryLimits::default()
    };
    let mut budget = DiscoveryBudget::new(limits);
    assert!(
        budget.admit(DiscoveryLimit::FactBytes, usize::MAX),
        "the exact payload limit is admitted"
    );
    assert!(
        !budget.admit(DiscoveryLimit::FactBytes, 1),
        "the next byte cannot wrap the counter"
    );
    assert!(
        budget.facts_exhausted(),
        "a parsed payload cut stops physical capture"
    );
    assert!(
        !budget.replay_exhausted(),
        "capture cuts preserve independently admitted replay work"
    );
    assert!(
        budget.admit(DiscoveryLimit::ReplayedBytes, usize::MAX),
        "replay also admits its exact boundary"
    );
    assert!(
        !budget.admit(DiscoveryLimit::ReplayedBytes, 1),
        "replay rejects counter overflow"
    );
    assert!(
        budget.replay_exhausted(),
        "the replay cut is explicit in its own phase"
    );
}
