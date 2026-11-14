// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Required-input handles and intrinsic conversion coordinates.

use p4kx_extraction::{Exclusion, InputScope, SourceUseDescription, SourceUseRole};

use crate::origin_capture::OriginSite;
use crate::required_input_fault_support::{self as support, Inventory, TestResult};

#[test]
fn required_input_receipt_keeps_original_handle_after_actual_read_without_wire_metadata()
-> TestResult {
    use std::io::Read as _;
    support::with_fixture(
        support::entries(),
        Inventory::Intact,
        &Exclusion::default(),
        |context, writer, _, _| {
            let target = support::lookup(context, "owner.dds.1", writer)?;
            context.with_required_input(
                &target,
                &crate::Subresource::new("synthetic", vec![0]),
                writer,
                |input, writer| {
                    let mut bytes = Vec::new();
                    input
                        .read_to_end(&mut bytes)
                        .map_err(|cause| crate::MediaError::Io {
                            path: "synthetic required input".into(),
                            source: cause,
                        })?;
                    assert_eq!(bytes, b"needed");
                    let original = input.origin_input(writer)?;
                    let mut capture = writer.capture_origins();
                    capture.add(
                        &original,
                        InputScope::Payload,
                        OriginSite::new("synthetic", &[0], "decoded"),
                        SourceUseRole::Conversion,
                        writer,
                    )?;
                    let origin = capture.finish().field_origin();
                    assert_eq!(origin.contains_encrypted_source(), Some(false));
                    assert!(origin.source_set_complete());
                    let receipt = input.receipt().clone();
                    assert!(receipt.origin.as_ref().is_some_and(|actual| {
                        actual.owner.is_same_inventory(&writer.origins.owner)
                    }));
                    assert!(
                        serde_json::to_value(receipt)?.get("origin").is_none(),
                        "the original payload contract remains byte-compatible"
                    );
                    Ok(())
                },
            )?;
            Ok(())
        },
    )
}

#[test]
fn required_target_digest_does_not_acquire_its_calling_subject_use_coordinates() -> TestResult {
    support::with_fixture(
        support::entries(),
        Inventory::Intact,
        &Exclusion::default(),
        |context, writer, _, _| {
            let target = support::lookup(context, "owner.dds.1", writer)?;
            let mut counts = Vec::new();
            for namespace in ["subject-first", "subject-second"] {
                context.with_required_input(
                    &target,
                    &crate::Subresource::new(namespace, vec![2]),
                    writer,
                    |input, writer| {
                        counts.push(target_conversion_count(input, writer)?);
                        Ok(())
                    },
                )?;
            }
            assert_eq!(
                counts.first(),
                counts.get(1),
                "reusing the same actual target does not invent another intrinsic conversion"
            );
            Ok(())
        },
    )
}

fn target_conversion_count(
    input: &crate::RequiredInput<'_>,
    writer: &crate::MediaWriter<'_>,
) -> Result<usize, crate::MediaError> {
    let target = input.origin_input(writer)?;
    let inventory = writer
        .origins
        .owner
        .freeze()
        .map_err(crate::origin_error::evidence)?;
    let conversions: Vec<_> = inventory
        .uses()
        .iter()
        .filter(|usage| {
            usage.source() == target.scope.source_ref() && usage.role() == SourceUseRole::Conversion
        })
        .collect();
    assert_intrinsic_sites(&conversions);
    Ok(conversions.len())
}

fn assert_intrinsic_sites(conversions: &[&SourceUseDescription]) {
    assert!(
        conversions.iter().any(|usage| matches!(
            usage.location(),
            p4kx_extraction::SourceLocation::Subresource { namespace, indices, field }
                if namespace == "media-physical-input"
                    && indices.is_empty() && field == "complete-payload"
        )),
        "target digest has its actual input-owned site"
    );
    assert!(
        conversions.iter().all(|usage| !matches!(
            usage.location(),
            p4kx_extraction::SourceLocation::Subresource { namespace, .. }
                if namespace == "subject-first" || namespace == "subject-second"
        )),
        "calling subject coordinates describe the relationship only"
    );
}
