// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::origin_input::MediaOriginInput;
use crate::{
    CandidateInventory, CandidateState, MediaError, MediaWriter, NotSelectedReason,
    ReferenceResolution, ReferenceUnavailable, SourceContext,
};

use super::test_support::{self as support, Counts, Probe, TestResult};

struct Evidence {
    ordinal: u64,
    input: MediaOriginInput,
}

fn evidence(
    context: &SourceContext<'_>,
    writer: &MediaWriter<'_>,
    ordinal: u64,
) -> Result<Evidence, MediaError> {
    Ok(Evidence {
        ordinal,
        input: context.origin_input(writer)?,
    })
}

fn assert_original(
    value: &Evidence,
    ordinal: u64,
    context: &SourceContext<'_>,
    writer: &MediaWriter<'_>,
) -> TestResult {
    assert_eq!(value.ordinal, ordinal);
    assert!(value.input.owner.is_same_inventory(&writer.origins.owner));
    let observed = context
        .directory
        .index
        .archive
        .observed_entry(context.entry)
        .ok_or("observed original")?;
    assert!(value.input.scope.matches_entry(&observed)?);
    Ok(())
}

#[test]
fn observed_admission_keeps_move_only_original_evidence_in_exact_candidate_order() -> TestResult {
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
            vec![first.clone()],
            vec![first, second],
            vec![excluded],
            vec![unread],
        ] {
            for inventory in [
                CandidateInventory::Complete,
                CandidateInventory::Unavailable(ReferenceUnavailable::IncompleteInventory),
            ] {
                let expected = writer.try_resolve_reference(
                    support::original()?,
                    candidates.iter().cloned().map(Ok),
                    inventory,
                )?;
                let count = candidates.len();
                let supplied = candidates
                    .iter()
                    .cloned()
                    .enumerate()
                    .map(|(index, candidate)| {
                        let ordinal = if index == 0 { 7 } else { 3 };
                        evidence(context, writer, ordinal).map(|origin| (candidate, origin))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let (actual, observed) = writer.try_resolve_reference_observed(
                    support::original()?,
                    supplied.into_iter().map(Ok),
                    inventory,
                )?;
                assert_eq!(
                    serde_json::to_value(actual)?,
                    serde_json::to_value(expected)?
                );
                assert_eq!(observed.retained.len(), count);
                assert!(observed.discarded.is_none());
                assert!(observed.inventory_used);
                for (index, value) in observed.retained.iter().enumerate() {
                    assert_original(value, if index == 0 { 7 } else { 3 }, context, writer)?;
                }
            }
        }
        Ok(())
    })
}

#[test]
fn empty_candidate_scans_consult_both_complete_and_unavailable_inventory() -> TestResult {
    support::with_writer(|_, writer| {
        for inventory in [
            CandidateInventory::Complete,
            CandidateInventory::Unavailable(ReferenceUnavailable::ProviderMissing),
        ] {
            let (reference, observed) = writer.try_resolve_reference_observed::<Evidence>(
                support::original()?,
                std::iter::empty(),
                inventory,
            )?;
            assert_eq!(reference.observed_candidates(), 0);
            assert!(observed.retained.is_empty());
            assert!(observed.discarded.is_none());
            assert!(observed.inventory_used);
            match inventory {
                CandidateInventory::Complete => assert!(matches!(
                    reference.resolution(),
                    ReferenceResolution::Missing
                )),
                CandidateInventory::Unavailable(_) => assert!(matches!(
                    reference.resolution(),
                    ReferenceResolution::Unavailable {
                        reason: ReferenceUnavailable::ProviderMissing,
                        ..
                    }
                )),
            }
        }
        Ok(())
    })
}

#[test]
fn count_cutoff_returns_only_retained_and_single_overflow_original_evidence() -> TestResult {
    support::with_writer(|context, writer| {
        writer.limits.reference_candidates = 1;
        let first = support::candidate(context, 0);
        let mut overflow = support::candidate(context, 1);
        overflow.target.root.hex.push('0');
        let first_evidence = evidence(context, writer, 7)?;
        let overflow_evidence = evidence(context, writer, 3)?;
        let counts = Counts::default();
        let (reference, observed) = writer.try_resolve_reference_observed(
            support::original()?,
            Probe::new(
                vec![
                    Ok((first, first_evidence)),
                    Ok((overflow, overflow_evidence)),
                    Err(MediaError::Contract("unconsumed tail".to_owned())),
                ],
                &counts,
            ),
            CandidateInventory::Unavailable(ReferenceUnavailable::ProviderMissing),
        )?;
        assert_eq!(reference.observed_candidates(), 2);
        assert!(
            matches!(reference.resolution(), ReferenceResolution::Unavailable { reason: ReferenceUnavailable::CandidateLimit { limit: 1 }, known_candidates } if known_candidates.len() == 1)
        );
        assert_eq!(observed.retained.len(), 1);
        assert_original(
            observed.retained.first().ok_or("retained evidence")?,
            7,
            context,
            writer,
        )?;
        assert_original(
            observed.discarded.as_ref().ok_or("overflow evidence")?,
            3,
            context,
            writer,
        )?;
        assert!(!observed.inventory_used);
        assert_eq!(counts.advanced.get(), 2);
        Ok(())
    })
}

#[test]
fn byte_cutoff_returns_the_validated_overflow_evidence_without_consulting_inventory() -> TestResult
{
    support::with_writer(|context, writer| {
        let original = support::original()?;
        let first = support::candidate(context, 0);
        let mut overflow = support::candidate(context, 1);
        overflow
            .target
            .subresource
            .as_mut()
            .ok_or("subresource")?
            .namespace = "wide".repeat(128);
        let limit =
            support::bytes(&original)? + support::bytes(&first)? + support::bytes(&overflow)? - 1;
        writer.limits.record_bytes = limit;
        let first_evidence = evidence(context, writer, 7)?;
        let overflow_evidence = evidence(context, writer, 3)?;
        let counts = Counts::default();
        let (reference, observed) = writer.try_resolve_reference_observed(
            original,
            Probe::new(
                vec![
                    Ok((first, first_evidence)),
                    Ok((overflow, overflow_evidence)),
                    Err(MediaError::Contract("unconsumed tail".to_owned())),
                ],
                &counts,
            ),
            CandidateInventory::Unavailable(ReferenceUnavailable::ProviderMissing),
        )?;
        assert_eq!(reference.observed_candidates(), 2);
        assert!(
            matches!(reference.resolution(), ReferenceResolution::Unavailable { reason: ReferenceUnavailable::CandidateBytes { limit: actual }, known_candidates } if *actual == limit && known_candidates.len() == 1)
        );
        assert_eq!(observed.retained.len(), 1);
        assert_original(
            observed.retained.first().ok_or("retained evidence")?,
            7,
            context,
            writer,
        )?;
        assert_original(
            observed.discarded.as_ref().ok_or("overflow evidence")?,
            3,
            context,
            writer,
        )?;
        assert!(!observed.inventory_used);
        assert_eq!(counts.advanced.get(), 2);
        Ok(())
    })
}

#[test]
fn identifier_byte_refusal_returns_no_evidence_and_does_not_initialize_the_scan() -> TestResult {
    support::with_writer(|context, writer| {
        let candidate = support::candidate(context, 0);
        let origin = evidence(context, writer, 7)?;
        writer.limits.record_bytes = 1;
        let counts = Counts::default();
        let (reference, observed) = writer.try_resolve_reference_observed(
            support::original()?,
            Probe::new(vec![Ok((candidate, origin))], &counts),
            CandidateInventory::Complete,
        )?;
        assert_eq!(reference.observed_candidates(), 0);
        assert!(
            matches!(reference.resolution(), ReferenceResolution::Unavailable { reason: ReferenceUnavailable::CandidateBytes { limit: 1 }, known_candidates } if known_candidates.is_empty())
        );
        assert!(observed.retained.is_empty());
        assert!(observed.discarded.is_none());
        assert!(!observed.inventory_used);
        assert_eq!(counts.initialized.get(), 0);
        assert_eq!(counts.advanced.get(), 0);
        Ok(())
    })
}

#[test]
fn observed_scan_errors_and_target_errors_keep_original_precedence() -> TestResult {
    support::with_writer(|context, writer| {
        let counts = Counts::default();
        let result = writer.try_resolve_reference_observed::<Evidence>(
            support::original()?,
            Probe::new(
                vec![Err(MediaError::Contract(
                    "original scan failure".to_owned(),
                ))],
                &counts,
            ),
            CandidateInventory::Unavailable(ReferenceUnavailable::IncompleteInventory),
        );
        assert!(
            matches!(result, Err(MediaError::Contract(detail)) if detail == "original scan failure")
        );
        assert_eq!(counts.advanced.get(), 1);
        let original = support::original()?;
        writer.limits.record_bytes = support::bytes(&original)?;
        let mut invalid = support::candidate(context, 0);
        invalid
            .target
            .subresource
            .as_mut()
            .ok_or("subresource")?
            .namespace
            .clear();
        let origin = evidence(context, writer, 7)?;
        let result = writer.try_resolve_reference_observed(
            original,
            [Ok((invalid, origin))],
            CandidateInventory::Complete,
        );
        assert!(
            matches!(result, Err(MediaError::Contract(detail)) if detail == "reference subresource has no namespace")
        );
        Ok(())
    })
}
