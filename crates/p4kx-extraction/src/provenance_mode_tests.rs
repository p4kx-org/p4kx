// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{ArchiveBuilder, TestEntry};
use serde_json::json;

use super::SourceUses;
use crate::{
    Archive, InputScope, ProvenanceMode, SourceCoordinateOmission, SourceEvidenceError,
    SourceInput, SourceLocation, SourceScope, SourceUseRole, SourceValue,
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[test]
fn marker_projection_matches_original_contributors_without_retaining_their_graph() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("plain.bin", b"plain"))
        .entry(TestEntry::stored("encrypted.bin", b"encrypted").encrypted())
        .write_to(&path)?;
    let archive = Archive::open_with_key(&path, Some(p4kx_testkit::test_key()))?;
    for mode in [ProvenanceMode::Full, ProvenanceMode::MarkersOnly] {
        let capture = SourceUses::with_provenance_mode(mode);
        assert_original_markers(&archive, &capture)?;
        let observed = capture.registered_use_count()?;
        let inventory = capture.finish_with(|| Ok::<(), SourceEvidenceError>(()))?;
        assert_eq!(inventory.provenance_mode(), mode, "typed inventory profile");
        if mode == ProvenanceMode::MarkersOnly {
            assert_eq!(observed, 1, "only the checked attempted use is retained");
            assert_eq!(inventory.source_count(), 0, "physical graph is withheld");
            assert_eq!(inventory.use_count(), 0, "use graph is withheld");
        } else {
            assert!(observed > 1, "the full capture retains actual uses");
            assert_eq!(inventory.use_count(), observed, "same registered full uses");
        }
    }
    Ok(())
}

fn assert_original_markers(archive: &Archive, capture: &SourceUses) -> TestResult {
    let plain = scope(archive, capture, "plain.bin")?;
    let encrypted = scope(archive, capture, "encrypted.bin")?;
    let unknown = capture.observe(SourceInput::Unknown, InputScope::Payload)?;
    let source = plain.at(location("value"), SourceUseRole::Payload)?;
    let selector = encrypted.at(location("selector"), SourceUseRole::Selector)?;
    let attempt = encrypted.at(location("unused"), SourceUseRole::Attempted)?;
    capture.verify_attempted_uses(std::slice::from_ref(attempt.reference()))?;
    let unrelated = encrypted.at(location("target"), SourceUseRole::Reference)?;
    let no_proof = unknown.at(location("external"), SourceUseRole::Conversion)?;
    let plain_origin = capture.origin(&[source, attempt, unrelated], true);
    assert_eq!(
        plain_origin.contains_encrypted_source(),
        Some(false),
        "attempts and unresolved references do not contribute"
    );
    let opaque = encrypted.at_opaque(
        SourceUseRole::Conversion,
        SourceCoordinateOmission::Unavailable,
    )?;
    let partial = capture.origin(&[opaque], true);
    assert_eq!(
        partial.contains_encrypted_source(),
        Some(true),
        "a known encrypted input survives unknown coordinates"
    );
    assert!(
        !partial.source_set_complete(),
        "an opaque coordinate is incomplete"
    );
    let unknown_origin = capture.origin(std::slice::from_ref(&no_proof), true);
    assert_eq!(
        unknown_origin.contains_encrypted_source(),
        None,
        "an external contribution stays unknown"
    );
    let combined = capture.origin_with_origins([&plain_origin, &unknown_origin, &partial], true)?;
    assert_eq!(
        combined.contains_encrypted_source(),
        Some(true),
        "encrypted wins in a union"
    );
    assert!(
        !combined.source_set_complete(),
        "the union retains incomplete input coverage"
    );
    assert_shared_marker(capture, selector, no_proof)
}

fn assert_shared_marker(
    capture: &SourceUses,
    selector: crate::SourceUse,
    unknown: crate::SourceUse,
) -> TestResult {
    let shared = capture.shared_origin(&[selector], true);
    let value =
        capture.value_with_shared_origin(SourceValue::Present(0), &shared, &[unknown], true)?;
    assert_eq!(
        value.origin().contains_encrypted_source(),
        Some(true),
        "an evaluated encrypted selector contributes to a shared field"
    );
    if capture.provenance_mode() == ProvenanceMode::MarkersOnly {
        assert_eq!(
            serde_json::to_value(&value)?,
            json!({"state":"present", "value":0, "containsEncryptedSource":true}),
            "a marker-only value retains its state and value with no provenance graph"
        );
    }
    Ok(())
}

#[test]
fn a_marker_capture_accepts_its_original_handles_and_rejects_foreign_input() -> TestResult {
    let first = SourceUses::with_provenance_mode(ProvenanceMode::MarkersOnly);
    let second = SourceUses::with_provenance_mode(ProvenanceMode::MarkersOnly);
    let a = first.observe(SourceInput::Unknown, InputScope::Payload)?;
    let b = second.observe(SourceInput::Unknown, InputScope::Payload)?;
    let selector = a.at(location("selector"), SourceUseRole::Selector)?;
    a.at_observed_location(&selector, SourceUseRole::Conversion)?;
    assert!(
        matches!(
            b.at_observed_location(&selector, SourceUseRole::Conversion),
            Err(SourceEvidenceError::ForeignEvidence)
        ),
        "equal source labels across captures confer no authority"
    );
    let foreign = first.origin(&[selector], true);
    assert!(
        matches!(
            second.origin_with_origins([&foreign], true),
            Err(SourceEvidenceError::ForeignEvidence)
        ),
        "marker unions reject foreign origins"
    );
    Ok(())
}

fn scope(archive: &Archive, capture: &SourceUses, name: &str) -> TestResult<SourceScope> {
    let entry = archive
        .entries()
        .iter()
        .find(|entry| entry.path() == name)
        .ok_or("original synthetic Entry")?;
    let observed = archive
        .observed_entry(entry)
        .ok_or("observed synthetic Entry")?;
    Ok(capture.observe(SourceInput::ArchiveView(observed), InputScope::Payload)?)
}

fn location(field: &str) -> SourceLocation {
    SourceLocation::DataForge {
        record_index: 0,
        field: field.into(),
    }
}
