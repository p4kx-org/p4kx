// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_testkit::media_fixture::{CaseOutcome, Choice, CoverageError, RunReport, SelectionError};

use super::observer::Fault;
use super::{Case, Identity, Prepared};

/// Runs a real canary without formatting private source-bearing errors.
#[expect(
    clippy::print_stderr,
    reason = "static technical failure categories identify canary failures"
)]
pub(crate) fn run(verify: impl FnOnce() -> super::Result<()>) {
    let result = verify();
    if let Err(error) = &result {
        eprintln!("media verification failed: {}", category(error.as_ref()));
    }
    assert!(result.is_ok(), "media_verification_failed");
}

/// Reports technical case outcomes and aggregate counts while retaining original errors.
#[expect(
    clippy::print_stdout,
    reason = "technical case outcomes and aggregate counts are the report"
)]
pub(crate) fn finish<T>(report: &RunReport<T>) -> super::Result<()> {
    for case in report.cases() {
        if let Err(error) = &case.outcome {
            println!(
                "media case {} failure: {}",
                case.case,
                category(error.as_ref())
            );
        }
    }
    println!("{report}");
    report.verify()?;
    Ok(())
}

/// Reports fulfilled capabilities without source names, values or identities.
#[expect(
    clippy::print_stdout,
    reason = "capability categories explain fixture selection"
)]
pub(crate) fn selected(case: &Case, choice: &Choice<Identity, Prepared>) {
    let facts = choice.capabilities.facts();
    let verified = |requested| {
        if requested {
            "verified"
        } else {
            "not_requested"
        }
    };
    println!(
        "media case {} selection: {:?}; geometry={}; lod_policy={:?}; size={}; interior={}; audio={}",
        case.label,
        choice.reason,
        verified(case.requirements.geometry_lod.is_some()),
        facts.lod_evidence(),
        verified(case.requirements.item_size.is_some()),
        verified(case.requirements.requires_interior),
        verified(case.requirements.requires_audio)
    );
}
fn category(mut error: &(dyn Error + 'static)) -> &'static str {
    for _ in 0..64 {
        if let Some(selection) = error.downcast_ref::<SelectionError<Identity>>() {
            match selection {
                SelectionError::DuplicateIdentity(_) => {
                    return "duplicate_physical_fixture_identity";
                }
                SelectionError::AmbiguousPreferred { .. } => return "ambiguous_preferred_class",
                SelectionError::IncompatiblePreferred { .. } => {
                    return "preferred_capability_mismatch";
                }
                SelectionError::Probe { source, .. } => {
                    error = source.as_ref();
                    continue;
                }
                _ => return "fixture_selection_failure",
            }
        }
        if let Some(fault) = error.downcast_ref::<Fault>() {
            return fault.category();
        }
        if super::is_invariant(error) {
            return "verification_invariant";
        }
        if error.is::<CoverageError>() {
            return "incomplete_case_coverage";
        }
        if error.is::<p4kx_extraction::ArchiveChanged>() {
            return "source_archive_changed";
        }
        if error.is::<p4kx_extraction::DataForgeError>() {
            return "canonical_source_failure";
        }
        if error.is::<p4kx_assembly::LoadoutError>() {
            return "loadout_projection_failure";
        }
        if error.is::<std::io::Error>() {
            return "required_io_failure";
        }
        if let Some(source) = error.source() {
            error = source;
        } else {
            break;
        }
    }
    "media_operation_failure"
}

/// Creates an explicit unavailable-fixture outcome.
pub(crate) fn absent<T>() -> CaseOutcome<T> {
    CaseOutcome::Absent("no source satisfies this documented capability scope".to_owned())
}
