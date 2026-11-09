// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::{
    CandidateInventory, CandidateState, MediaError, NotSelectedReason, ReferenceResolution,
    ReferenceUnavailable,
};

use super::test_support::{self as support, Counts, Probe, TestResult};

#[test]
fn identifier_refusal_and_global_query_admission_do_not_initialize_fallible_candidates()
-> TestResult {
    support::with_writer(|_, writer| {
        writer.limits.record_bytes = 1;
        writer.limits.references = 1;
        let counts = Counts::default();
        let reference = writer.try_resolve_reference(
            support::original()?,
            Probe::new(
                vec![Err(MediaError::Contract("unconsumed scan".to_owned()))],
                &counts,
            ),
            CandidateInventory::Unavailable(ReferenceUnavailable::ProviderMissing),
        )?;
        assert_eq!(reference.observed_candidates(), 0);
        assert!(!reference.candidates_complete());
        assert!(
            matches!(reference.resolution(), ReferenceResolution::Unavailable {
            reason: ReferenceUnavailable::CandidateBytes { limit: 1 }, known_candidates,
        } if known_candidates.is_empty()),
            "identifier bytes are refused before candidate scanning"
        );
        assert_eq!(counts.initialized.get(), 0);
        assert_eq!(counts.advanced.get(), 0);
        let result = writer.try_resolve_reference(
            support::original()?,
            Probe::new(
                vec![Err(MediaError::Contract("unconsumed scan".to_owned()))],
                &counts,
            ),
            CandidateInventory::Complete,
        );
        assert!(
            matches!(
                result,
                Err(MediaError::Limit {
                    budget: "references",
                    limit: 1
                })
            ),
            "the refused identifier still consumed exactly one query admission"
        );
        assert_eq!(counts.initialized.get(), 0);
        assert_eq!(counts.advanced.get(), 0);
        Ok(())
    })
}

#[test]
fn public_reference_methods_preserve_their_distinct_iterator_initialization_order() -> TestResult {
    for fallible in [false, true] {
        support::with_writer(|context, writer| {
            writer.limits.record_bytes = 1;
            let counts = Counts::default();
            let value = support::candidate(context, 0);
            let reference = if fallible {
                writer.try_resolve_reference(
                    support::original()?,
                    Probe::new(vec![Ok(value)], &counts),
                    CandidateInventory::Complete,
                )?
            } else {
                writer.resolve_reference(
                    support::original()?,
                    Probe::new(vec![value], &counts),
                    CandidateInventory::Complete,
                )?
            };
            assert_eq!(reference.observed_candidates(), 0);
            assert_eq!(
                counts.initialized.get(),
                usize::from(!fallible),
                "only the infallible facade initializes its iterator before identifier admission"
            );
            assert_eq!(counts.advanced.get(), 0);
            Ok(())
        })?;
    }
    Ok(())
}

#[test]
fn candidate_count_cutoff_sees_one_overflow_without_validating_it_or_reading_the_tail() -> TestResult
{
    support::with_writer(|context, writer| {
        writer.limits.reference_candidates = 1;
        let first = support::candidate(context, 0);
        let mut overflow = support::candidate(context, 1);
        overflow.target.root.hex.push('0');
        let counts = Counts::default();
        let reference = writer.try_resolve_reference(
            support::original()?,
            Probe::new(
                vec![
                    Ok(first),
                    Ok(overflow),
                    Err(MediaError::Contract("unconsumed tail".to_owned())),
                ],
                &counts,
            ),
            CandidateInventory::Unavailable(ReferenceUnavailable::ProviderMissing),
        )?;
        assert_eq!(reference.observed_candidates(), 2);
        assert!(!reference.candidates_complete());
        assert!(
            matches!(reference.resolution(), ReferenceResolution::Unavailable {
            reason: ReferenceUnavailable::CandidateLimit { limit: 1 }, known_candidates,
        } if known_candidates.len() == 1),
            "the count cutoff precedes overflow target validation and inventory disposition"
        );
        assert_eq!(counts.initialized.get(), 1);
        assert_eq!(counts.advanced.get(), 2);
        Ok(())
    })
}

#[test]
fn candidate_byte_cutoff_retains_only_the_admitted_prefix_and_does_not_read_the_tail() -> TestResult
{
    support::with_writer(|context, writer| {
        let original = support::original()?;
        let first = support::candidate(context, 0);
        let mut overflow = support::candidate(context, 1);
        overflow
            .target
            .subresource
            .as_mut()
            .ok_or("candidate subresource")?
            .namespace = "wide".repeat(128);
        let limit =
            support::bytes(&original)? + support::bytes(&first)? + support::bytes(&overflow)? - 1;
        writer.limits.record_bytes = limit;
        let counts = Counts::default();
        let reference = writer.try_resolve_reference(
            original,
            Probe::new(
                vec![
                    Ok(first),
                    Ok(overflow),
                    Err(MediaError::Contract("unconsumed tail".to_owned())),
                ],
                &counts,
            ),
            CandidateInventory::Unavailable(ReferenceUnavailable::ProviderMissing),
        )?;
        assert_eq!(reference.observed_candidates(), 2);
        assert!(!reference.candidates_complete());
        assert!(
            matches!(reference.resolution(), ReferenceResolution::Unavailable {
            reason: ReferenceUnavailable::CandidateBytes { limit: actual }, known_candidates,
        } if *actual == limit && known_candidates.len() == 1),
            "byte refusal precedes the unused inventory disposition"
        );
        assert_eq!(counts.initialized.get(), 1);
        assert_eq!(counts.advanced.get(), 2);
        Ok(())
    })
}

#[test]
fn admitted_target_validation_precedes_candidate_byte_refusal() -> TestResult {
    support::with_writer(|context, writer| {
        let original = support::original()?;
        writer.limits.record_bytes = support::bytes(&original)?;
        let mut invalid = support::candidate(context, 0);
        invalid
            .target
            .subresource
            .as_mut()
            .ok_or("candidate subresource")?
            .namespace
            .clear();
        let counts = Counts::default();
        let result = writer.try_resolve_reference(
            original,
            Probe::new(
                vec![
                    Ok(invalid),
                    Err(MediaError::Contract("unconsumed tail".to_owned())),
                ],
                &counts,
            ),
            CandidateInventory::Complete,
        );
        assert!(
            matches!(result, Err(MediaError::Contract(detail)) if detail == "reference subresource has no namespace"),
            "target validation keeps its original precedence over byte admission"
        );
        assert_eq!(counts.initialized.get(), 1);
        assert_eq!(counts.advanced.get(), 1);
        Ok(())
    })
}

#[test]
fn original_iterator_errors_are_not_converted_to_unavailable_inventory() -> TestResult {
    support::with_writer(|context, writer| {
        let counts = Counts::default();
        let result = writer.try_resolve_reference(
            support::original()?,
            Probe::new(
                vec![
                    Err(MediaError::Contract("original scan failure".to_owned())),
                    Ok(support::candidate(context, 0)),
                ],
                &counts,
            ),
            CandidateInventory::Unavailable(ReferenceUnavailable::IncompleteInventory),
        );
        assert!(
            matches!(result, Err(MediaError::Contract(detail)) if detail == "original scan failure")
        );
        assert_eq!(counts.initialized.get(), 1);
        assert_eq!(counts.advanced.get(), 1);
        Ok(())
    })
}

#[test]
fn public_infallible_and_fallible_reference_results_preserve_every_classification() -> TestResult {
    support::with_writer(|context, writer| {
        let first = support::candidate(context, 0);
        let second = support::candidate(context, 1);
        let mut excluded = first.clone();
        excluded.state = CandidateState::NotSelected {
            reason: NotSelectedReason::Filter,
        };
        let mut unread = first.clone();
        unread.state = CandidateState::Unavailable {
            reason: ReferenceUnavailable::TargetNotRead,
        };
        for candidates in [
            Vec::new(),
            vec![first.clone()],
            vec![first, second],
            vec![excluded],
            vec![unread],
        ] {
            for inventory in [
                CandidateInventory::Complete,
                CandidateInventory::Unavailable(ReferenceUnavailable::IncompleteInventory),
            ] {
                let plain = writer.resolve_reference(
                    support::original()?,
                    candidates.clone(),
                    inventory,
                )?;
                let fallible = writer.try_resolve_reference(
                    support::original()?,
                    candidates.clone().into_iter().map(Ok),
                    inventory,
                )?;
                assert_eq!(
                    serde_json::to_value(&plain)?,
                    serde_json::to_value(&fallible)?,
                    "both existing public facades preserve the exact wire result"
                );
            }
        }
        Ok(())
    })
}
