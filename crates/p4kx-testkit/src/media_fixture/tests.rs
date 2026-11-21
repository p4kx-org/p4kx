// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use super::{Candidate, CaseOutcome, RunReport, SelectionError, SelectionReason};

type TestResult = Result<(), Box<dyn Error>>;

fn candidate(identity: u32, name: &str) -> Candidate<u32> {
    Candidate {
        identity,
        class_name: name.to_owned(),
    }
}

#[test]
fn a_present_preference_is_used_before_any_alternative() -> TestResult {
    let candidates = [
        candidate(1, "Family.Alternative"),
        candidate(9, "Family.Preferred"),
    ];
    let mut inspected = Vec::new();
    let choice = super::select(
        &["Family.Preferred"],
        &candidates,
        |_| true,
        |candidate| {
            inspected.push(candidate.identity);
            Ok(Some("geometry, requested LOD"))
        },
    )?
    .ok_or("selection absent")?;
    assert_eq!(
        choice.candidate.identity, 9,
        "the documented preferred case stays stable"
    );
    assert_eq!(
        choice.reason,
        SelectionReason::Preferred,
        "the selection reason is explicit"
    );
    assert_eq!(
        inspected,
        [9],
        "selection does not probe irrelevant alternatives"
    );
    Ok(())
}

#[test]
fn missing_and_renamed_preferences_choose_capable_alternatives_deterministically() -> TestResult {
    for reversed in [false, true] {
        let mut candidates = vec![
            candidate(8, "Family.Renamed"),
            candidate(3, "Family.Other"),
            candidate(1, "Other"),
        ];
        if reversed {
            candidates.reverse();
        }
        let choice = super::select(
            &["Family.Missing"],
            &candidates,
            |candidate| candidate.class_name.starts_with("Family."),
            |candidate| Ok((candidate.identity != 3).then_some("required source geometry")),
        )?
        .ok_or("fallback absent")?;
        assert_eq!(
            choice.candidate.identity, 8,
            "capability requirements exclude smaller or undeclared sources"
        );
        assert_eq!(
            choice.reason,
            SelectionReason::Fallback,
            "fallback is never described as the original fixture"
        );
    }
    Ok(())
}

#[test]
fn ambiguous_preferred_classes_fail_without_fallback() {
    let candidates = [
        candidate(7, "Preferred"),
        candidate(3, "preferred"),
        candidate(1, "Other"),
    ];
    let result = super::select(&["Preferred"], &candidates, |_| true, |_| Ok(Some(())));
    assert!(
        matches!(result, Err(SelectionError::AmbiguousPreferred { identities, .. }) if identities == [3, 7]),
        "all physical aliases make the preference ambiguous, regardless of input order"
    );
}

#[test]
fn a_present_incompatible_preference_fails_without_replacement() {
    let candidates = [candidate(1, "Preferred"), candidate(2, "Other")];
    let result = super::select(
        &["Preferred"],
        &candidates,
        |_| true,
        |candidate| Ok((candidate.identity == 2).then_some(())),
    );
    assert!(
        matches!(
            result,
            Err(SelectionError::IncompatiblePreferred { identity: 1 })
        ),
        "present source drift is a regression, not unavailable coverage"
    );
}

#[test]
fn actual_preferred_and_fallback_probe_errors_never_trigger_replacement() {
    for preferred in [["Preferred"].as_slice(), ["Missing"].as_slice()] {
        let candidates = [candidate(1, "Preferred"), candidate(2, "Alternative")];
        let mut inspected = Vec::new();
        let result = super::select(
            preferred,
            &candidates,
            |_| true,
            |candidate| {
                inspected.push(candidate.identity);
                if candidate.identity == 1 {
                    Err("actual parse fault".into())
                } else {
                    Ok(Some(()))
                }
            },
        );
        assert!(
            matches!(result, Err(SelectionError::Probe { identity: 1, .. })),
            "the actual selected probe error remains a failure"
        );
        assert_eq!(
            inspected,
            [1],
            "a broken source never causes endless fallback"
        );
    }
}

#[test]
fn an_unavailable_category_is_explicitly_absent() -> TestResult {
    let candidates = [candidate(1, "Other")];
    let choice = super::select(&["Missing"], &candidates, |_| false, |_| Ok(Some(())))?;
    assert!(
        choice.is_none(),
        "an absent eligible category produces no fabricated fixture"
    );
    Ok(())
}

#[test]
fn removing_a_preference_does_not_prevent_later_cases_from_running() -> TestResult {
    let candidates = [candidate(2, "Later")];
    let mut report = RunReport::default();
    for name in ["Missing", "Later"] {
        report.run(name, || {
            match super::select(
                &[name],
                &candidates,
                |candidate| candidate.class_name == name,
                |_| Ok(Some(())),
            )? {
                Some(choice) => Ok(CaseOutcome::Completed(choice.candidate.identity)),
                None => Ok(CaseOutcome::Absent(
                    "no compatible source declarations".to_owned(),
                )),
            }
        });
    }
    assert_eq!(
        report.cases().len(),
        2,
        "every requested case appears in the coverage inventory"
    );
    assert!(
        matches!(&report.cases()[0].outcome, Ok(CaseOutcome::Absent(_))),
        "missing preference is distinguishable from a conversion failure"
    );
    assert!(
        matches!(&report.cases()[1].outcome, Ok(CaseOutcome::Completed(2))),
        "later cases still execute"
    );
    report.verify()?;
    Ok(())
}

#[test]
fn a_selected_conversion_failure_fails_the_canary_after_running_later_cases() {
    let mut report = RunReport::default();
    report.run(
        "selected broken export",
        || -> Result<CaseOutcome<u32>, Box<dyn Error>> { Err("selected conversion failed".into()) },
    );
    report.run("later healthy export", || Ok(CaseOutcome::Completed(2)));
    let result = report.verify();
    assert!(
        matches!(result, Err(error) if error.failed == 1 && error.completed == 1),
        "selected conversion failures remain fatal while later coverage is recorded"
    );
    assert_eq!(
        report.cases().len(),
        2,
        "one failure never truncates the case inventory"
    );
}

#[test]
fn all_absent_cases_never_claim_meaningful_coverage() {
    let mut report = RunReport::<u32>::default();
    report.run("absent", || {
        Ok(CaseOutcome::Absent("no eligible family".to_owned()))
    });
    assert!(
        matches!(report.verify(), Err(error) if error.completed == 0 && error.absent == 1),
        "no completed conversion must fail instead of passing a hidden coverage hole"
    );
}

#[test]
fn repeated_physical_identities_fail_before_any_probe() {
    let candidates = [candidate(1, "First"), candidate(1, "Renamed")];
    let mut probes = 0;
    let result = super::select(
        &["First"],
        &candidates,
        |_| true,
        |_| {
            probes += 1;
            Ok(Some(()))
        },
    );
    assert!(
        matches!(result, Err(SelectionError::DuplicateIdentity(1))),
        "one physical candidate cannot claim two independent identities"
    );
    assert_eq!(
        probes, 0,
        "ambiguous inventory is rejected before media parsing"
    );
}

#[test]
fn reports_keep_original_payloads_and_errors_without_formatting_them() -> TestResult {
    struct Payload(&'static str);
    let mut report = RunReport::default();
    report.run("model_case", || {
        Ok(CaseOutcome::Completed(Payload("PRIVATE_VALUE_SENTINEL")))
    });
    report.run("audio_case", || {
        Ok(CaseOutcome::Absent("PRIVATE_PATH_SENTINEL".to_owned()))
    });
    report.run("interior_case", || {
        Err(Box::new(std::io::Error::other("PRIVATE_ERROR_SENTINEL")))
    });
    let rendered = report.to_string();
    assert!(
        !rendered.contains("PRIVATE_"),
        "reports expose only technical labels and aggregate outcomes"
    );
    assert!(
        rendered.contains("model_case: completed") && rendered.contains("interior_case: failed"),
        "privacy does not hide completed or failed case coverage"
    );
    let first = report.cases().first().ok_or("completed case absent")?;
    assert!(
        matches!(
            &first.outcome,
            Ok(CaseOutcome::Completed(Payload("PRIVATE_VALUE_SENTINEL")))
        ),
        "the original completion payload remains available internally"
    );
    let failed = report.cases().get(2).ok_or("failed case absent")?;
    let source = failed
        .outcome
        .as_ref()
        .err()
        .ok_or("typed failure absent")?;
    assert!(
        source.downcast_ref::<std::io::Error>().is_some(),
        "sanitized output does not replace the original producer error"
    );
    Ok(())
}

#[test]
fn selection_error_display_does_not_expose_source_identity_or_reason() -> TestResult {
    let error = SelectionError::Probe {
        identity: "PRIVATE_IDENTITY_SENTINEL".to_owned(),
        source: Box::new(std::io::Error::other("PRIVATE_SOURCE_SENTINEL")),
    };
    assert_eq!(
        error.to_string(),
        "fixture_probe_failure",
        "the public display contains a stable technical failure category"
    );
    assert_eq!(
        error.source().ok_or("original source absent")?.to_string(),
        "PRIVATE_SOURCE_SENTINEL",
        "the original source remains available through the error chain"
    );
    Ok(())
}
