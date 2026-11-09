// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Capture cuts and actual contributing-use retention.

use std::error::Error;

use p4kx_extraction::{Exclusion, InputScope, SourceUseRole};
use p4kx_testkit::TestEntry;

use crate::origin_capture::OriginSite;
use crate::origin_input::MediaOriginInput;
use crate::required_input_fault_support::{self as support, Inventory, TestResult};

#[path = "origin_capture_prepared_tests.rs"]
mod prepared;
#[path = "origin_capture_required_tests.rs"]
mod required;

fn entries() -> [TestEntry; 2] {
    [
        TestEntry::stored("owner.dds", b"H"),
        TestEntry::stored("unread.bin", b"opaque").flagged_encrypted(),
    ]
}

#[test]
fn minimal_original_proof_keeps_encryption_when_optional_details_are_not_captured() -> TestResult {
    support::with_fixture(
        entries(),
        Inventory::Intact,
        &Exclusion::default(),
        |context, writer, _, _| {
            let input = encrypted_input(context, writer)?;
            let mut capture = writer.capture_origins();
            capture.add(
                &input,
                InputScope::Payload,
                OriginSite::new("synthetic", &[], "value"),
                SourceUseRole::Payload,
                writer,
            )?;
            let contribution = capture.finish();
            let origin = contribution.field_origin();
            assert_eq!(origin.contains_encrypted_source(), Some(true));
            assert!(
                origin.source_set_complete(),
                "optional path loss does not discard proof"
            );
            assert_eq!(
                contribution.cuts.as_ref(),
                &[crate::origin_contributions::OriginCut::InputMetadata]
            );
            let inventory = serde_json::to_value(writer.origins.owner.freeze()?)?;
            assert!(
                inventory
                    .get("sources")
                    .and_then(serde_json::Value::as_array)
                    .ok_or("original source inventory")?
                    .iter()
                    .any(
                        |source| source.get("kind").and_then(serde_json::Value::as_str)
                            == Some("archiveEntryProof")
                            && source
                                .get("storedEncryption")
                                .and_then(serde_json::Value::as_str)
                                == Some("encrypted")
                    )
            );
            Ok(())
        },
    )
}

#[test]
fn a_late_encrypted_witness_survives_local_capture_cut_and_the_next_field_recovers() -> TestResult {
    support::with_fixture(
        entries(),
        Inventory::Intact,
        &Exclusion::default(),
        |context, writer, _, _| {
            let plain = context.origin_input(writer)?;
            let encrypted = encrypted_input(context, writer)?;
            writer.limits.record_bytes = 1;
            let mut capture = writer.capture_origins();
            for input in [&plain, &encrypted] {
                capture.add(
                    input,
                    InputScope::Payload,
                    OriginSite::new("synthetic", &[], "value"),
                    SourceUseRole::Conversion,
                    writer,
                )?;
            }
            let origin = capture.finish().field_origin();
            assert_eq!(
                origin.contains_encrypted_source(),
                Some(true),
                "later true is never lost"
            );
            assert!(
                !origin.source_set_complete(),
                "opaque coordinate and handle cut are explicit"
            );
            assert_eq!(
                origin.source_uses_refs().len(),
                1,
                "one original opaque witness remains"
            );
            writer.limits.record_bytes = crate::Limits::default().record_bytes;
            let mut later = writer.capture_origins();
            later.add(
                &plain,
                InputScope::Payload,
                OriginSite::new("synthetic", &[], "later"),
                SourceUseRole::Payload,
                writer,
            )?;
            let later = later.finish().field_origin();
            assert_eq!(later.contains_encrypted_source(), Some(false));
            assert!(
                later.source_set_complete(),
                "independent later fields have their own allowance"
            );
            assert_eq!(writer.limits().records, crate::Limits::default().records);
            Ok(())
        },
    )
}

#[test]
fn attempted_and_reference_targets_do_not_taint_the_retained_plain_value() -> TestResult {
    support::with_fixture(
        entries(),
        Inventory::Intact,
        &Exclusion::default(),
        |context, writer, _, _| {
            let plain = context.origin_input(writer)?;
            let encrypted = encrypted_input(context, writer)?;
            for plain_first in [true, false] {
                let mut capture = writer.capture_origins();
                if plain_first {
                    add_plain(&mut capture, &plain, writer)?;
                }
                add_noncontributors(&mut capture, &encrypted, writer)?;
                if !plain_first {
                    add_plain(&mut capture, &plain, writer)?;
                }
                let contributions = capture.finish();
                let origin = contributions.field_origin();
                assert_eq!(origin.contains_encrypted_source(), Some(false));
                assert!(origin.source_set_complete());
                assert_eq!(
                    origin.source_uses_refs().len(),
                    1,
                    "only the actual plain contributor"
                );
                assert!(
                    contributions.cuts.is_empty(),
                    "noncontributing optional metadata never cuts the retained field"
                );
            }
            let inventory = writer.origins.owner.freeze()?;
            assert!(
                inventory
                    .uses()
                    .iter()
                    .any(|usage| usage.role() == SourceUseRole::Attempted)
            );
            assert!(
                inventory
                    .uses()
                    .iter()
                    .any(|usage| usage.role() == SourceUseRole::Reference)
            );
            Ok(())
        },
    )
}

fn add_plain(
    capture: &mut crate::origin_capture::OriginCapture,
    input: &MediaOriginInput,
    writer: &crate::MediaWriter<'_>,
) -> TestResult {
    capture.add(
        input,
        InputScope::Payload,
        OriginSite::new("synthetic", &[], "value"),
        SourceUseRole::Payload,
        writer,
    )?;
    Ok(())
}

fn add_noncontributors(
    capture: &mut crate::origin_capture::OriginCapture,
    input: &MediaOriginInput,
    writer: &crate::MediaWriter<'_>,
) -> TestResult {
    let oversized = "x".repeat(usize::try_from(writer.limits().record_bytes)? + 1);
    for role in [SourceUseRole::Attempted, SourceUseRole::Reference] {
        for site in [
            OriginSite::new("synthetic", &[], "untouched"),
            OriginSite::new("", &[], "untouched"),
            OriginSite::new("synthetic", &[], &oversized),
        ] {
            capture.add(input, InputScope::Payload, site, role, writer)?;
        }
    }
    Ok(())
}

fn encrypted_input(
    context: &crate::SourceContext<'_>,
    writer: &crate::MediaWriter<'_>,
) -> Result<MediaOriginInput, Box<dyn Error + Send + Sync>> {
    let entry = context
        .directory
        .index
        .archive
        .entries()
        .get(1)
        .ok_or("second Entry")?;
    Ok(MediaOriginInput::observe(
        &writer.origins.owner,
        context.directory.index.archive,
        entry,
        0,
    )?)
}
