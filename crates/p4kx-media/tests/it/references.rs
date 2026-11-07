// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_media::{
    CandidateInventory, CandidateState, ExportOptions, IntegerWidth, MediaError, MediaKind,
    MediaWriter, NotSelectedReason, OriginalIdentifier, Reference, ReferenceCandidate,
    ReferenceTarget, ReferenceUnavailable, SourceContext, Subresource,
};

use super::providers::registry;
use super::support::{self, TestResult};

pub(super) fn cases(
    context: &SourceContext<'_>,
    writer: &mut MediaWriter<'_>,
) -> Result<Vec<Reference>, MediaError> {
    let mut references = selected_cases(context, writer)?;
    references.push(not_selected(context, writer)?);
    references.extend(unavailable_cases(context, writer)?);
    Ok(references)
}

fn selected_cases(
    context: &SourceContext<'_>,
    writer: &mut MediaWriter<'_>,
) -> Result<Vec<Reference>, MediaError> {
    let target = ReferenceTarget::new_with_subresource(
        context.source,
        MediaKind::Animation,
        Subresource::new("samples", vec![0]),
    );
    let next = ReferenceTarget::new_with_subresource(
        context.source,
        MediaKind::Animation,
        Subresource::new("samples", vec![1]),
    );
    let selected = |target| ReferenceCandidate::new(target, CandidateState::Selected);
    let resolved = writer.resolve_reference(
        OriginalIdentifier::new_with_guid_text("01234567-89AB-CDEF-0123-456789ABCDEF"),
        [selected(target.clone())],
        CandidateInventory::Complete,
    )?;
    let missing = writer.resolve_reference(
        OriginalIdentifier::new_with_path("Data\\Case/Unknown.MTL"),
        [],
        CandidateInventory::Complete,
    )?;
    let ambiguous = writer.resolve_reference(
        OriginalIdentifier::try_new_with_unsigned("synthetic.id", IntegerWidth::Bits64, u64::MAX)?,
        [selected(target), selected(next)],
        CandidateInventory::Complete,
    )?;
    Ok(vec![resolved, missing, ambiguous])
}

fn not_selected(
    context: &SourceContext<'_>,
    writer: &mut MediaWriter<'_>,
) -> Result<Reference, MediaError> {
    writer.resolve_reference(
        OriginalIdentifier::try_new_with_text("synthetic.kind", "audio")?,
        [ReferenceCandidate::new(
            ReferenceTarget::new_with_subresource(
                context.source,
                MediaKind::Audio,
                Subresource::new("audio", vec![0]),
            ),
            CandidateState::NotSelected {
                reason: NotSelectedReason::KindNotRequested,
            },
        )],
        CandidateInventory::Complete,
    )
}

fn unavailable_cases(
    context: &SourceContext<'_>,
    writer: &mut MediaWriter<'_>,
) -> Result<Vec<Reference>, MediaError> {
    let target = ReferenceTarget::new_with_source(context.source);
    let unavailable = writer.resolve_reference(
        OriginalIdentifier::try_new_with_signed("synthetic.id", IntegerWidth::Bits64, i64::MIN)?,
        [],
        CandidateInventory::Unavailable(ReferenceUnavailable::ProviderMissing),
    )?;
    let unread = writer.resolve_reference(
        OriginalIdentifier::try_new_with_text("synthetic.state", "unread")?,
        [ReferenceCandidate::new(
            target,
            CandidateState::Unavailable {
                reason: ReferenceUnavailable::TargetNotRead,
            },
        )],
        CandidateInventory::Complete,
    )?;
    let invalid = writer.resolve_reference(
        OriginalIdentifier::new_with_guid_text(" {DANGLING-not-a-guid} "),
        [],
        CandidateInventory::Unavailable(ReferenceUnavailable::InvalidIdentifier),
    )?;
    Ok(vec![unavailable, unread, invalid])
}

#[test]
fn both_providers_use_all_reference_states_and_preserve_exact_64_bit_identifiers() -> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = support::archive(folder.path())?;
    let output = folder.path().join("media");
    let index = support::published(p4kx_media::export_with(
        &archive,
        &output,
        &ExportOptions::default(),
        &registry()?,
    )?)?;
    let schema = p4kx_media::reference_schema();
    let validator = jsonschema::validator_for(&schema)?;
    for stream in ["material.jsonl", "animation.jsonl"] {
        let records = support::records(&output, &index, stream)?;
        let references = records
            .first()
            .and_then(|record| record.data.get("references"))
            .and_then(serde_json::Value::as_array)
            .ok_or("shared reference data")?;
        for reference in references {
            validator
                .validate(reference)
                .map_err(|error| error.to_string())?;
        }
        let statuses: Vec<_> = references
            .iter()
            .filter_map(|reference| {
                reference
                    .pointer("/resolution/status")
                    .and_then(serde_json::Value::as_str)
            })
            .collect();
        assert_eq!(
            statuses,
            [
                "resolved",
                "missing",
                "ambiguous",
                "not-selected",
                "unavailable",
                "unavailable",
                "unavailable"
            ]
        );
        assert_original_identifiers(references);
    }
    Ok(())
}

fn assert_original_identifiers(references: &[serde_json::Value]) {
    for (index, expected) in [
        (0, "01234567-89AB-CDEF-0123-456789ABCDEF"),
        (1, "Data\\Case/Unknown.MTL"),
        (2, "18446744073709551615"),
        (4, "-9223372036854775808"),
        (6, " {DANGLING-not-a-guid} "),
    ] {
        assert_eq!(
            references
                .get(index)
                .and_then(|reference| reference.pointer("/original/value"))
                .and_then(serde_json::Value::as_str),
            Some(expected),
            "source identifier spelling and full integer precision survive"
        );
    }
}

#[test]
fn candidate_limit_reports_only_observed_lower_bound_without_false_missing_or_resolved()
-> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = support::archive(folder.path())?;
    let output = folder.path().join("media");
    let mut options = ExportOptions::default();
    options.limits.reference_candidates = 1;
    let index = support::published(p4kx_media::export_with(
        &archive,
        &output,
        &options,
        &registry()?,
    )?)?;
    let records = support::records(&output, &index, "animation.jsonl")?;
    let reference = records
        .first()
        .and_then(|record| record.data.pointer("/references/2"))
        .ok_or("bounded ambiguity")?;
    assert_eq!(
        reference
            .pointer("/resolution/status")
            .and_then(serde_json::Value::as_str),
        Some("unavailable")
    );
    assert_eq!(
        reference
            .pointer("/resolution/reason/type")
            .and_then(serde_json::Value::as_str),
        Some("candidate-limit")
    );
    assert_eq!(
        reference
            .get("observed_candidates")
            .and_then(serde_json::Value::as_u64),
        Some(2)
    );
    assert_eq!(
        reference
            .get("candidates_complete")
            .and_then(serde_json::Value::as_bool),
        Some(false)
    );
    assert_eq!(
        reference
            .pointer("/resolution/known_candidates")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(1)
    );
    Ok(())
}

#[test]
fn integer_width_is_checked_without_truncation() {
    assert!(
        OriginalIdentifier::try_new_with_unsigned("synthetic.id", IntegerWidth::Bits8, 256)
            .is_err(),
        "unsigned source width cannot truncate"
    );
    assert!(
        OriginalIdentifier::try_new_with_signed("synthetic.id", IntegerWidth::Bits8, 128).is_err(),
        "signed source width cannot wrap"
    );
}
