// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Prepared record addresses and original input ownership.

use p4kx_extraction::{Exclusion, FieldPath, InputScope, SourceUseRole};

use crate::origin_capture::OriginSite;
use crate::origin_input::MediaOriginInput;
use crate::required_input_fault_support::{self as support, Inventory, TestResult};

#[test]
fn foreign_original_inventory_is_rejected_by_capture_include_and_prepared_fields() -> TestResult {
    support::with_fixture(
        super::entries(),
        Inventory::Intact,
        &Exclusion::default(),
        |context, writer, _, _| {
            let other = p4kx_extraction::SourceUses::new();
            let foreign = MediaOriginInput::observe(
                &other,
                context.directory.index.archive,
                context.entry,
                0,
            )?;
            let mut capture = writer.capture_origins();
            assert!(
                capture
                    .add(
                        &foreign,
                        InputScope::Payload,
                        OriginSite::new("synthetic", &[], "value"),
                        SourceUseRole::Payload,
                        writer
                    )
                    .is_err()
            );
            let mut other_capture = crate::origin_capture::OriginCapture::with_limit(
                &other,
                writer.limits().record_bytes,
            );
            other_capture.add_original(
                &foreign,
                InputScope::Payload,
                OriginSite::new("synthetic", &[], "value"),
                SourceUseRole::Payload,
            )?;
            let foreign = other_capture.finish();
            assert!(capture.include(&foreign, writer).is_err());
            let mut fields = writer.record_origins();
            assert!(
                fields
                    .insert(&FieldPath::parse("/data/value")?, foreign, writer)
                    .is_err()
            );
            Ok(())
        },
    )
}

#[test]
fn prepared_fields_preserve_escaped_empty_and_list_addresses_and_refuse_nonexistent_paths()
-> TestResult {
    support::with_fixture(
        super::entries(),
        Inventory::Intact,
        &Exclusion::default(),
        |context, writer, _, _| {
            let input = context.origin_input(writer)?;
            let mut capture = writer.capture_origins();
            capture.add(
                &input,
                InputScope::Payload,
                OriginSite::new("synthetic", &[], "value"),
                SourceUseRole::Payload,
                writer,
            )?;
            let contribution = capture.finish();
            let mut fields = writer.record_origins();
            for pointer in ["/data/", "/data/a~1b/~0/0"] {
                fields.insert(&FieldPath::parse(pointer)?, contribution.clone(), writer)?;
            }
            writer.record_with_origins(
                &crate::Subresource::new("synthetic", Vec::new()),
                crate::Status::Complete,
                &serde_json::json!({"": null, "a/b": {"~": [1]}}),
                &[],
                fields,
            )?;
            let mut missing = writer.record_origins();
            missing.insert(&FieldPath::parse("/data/missing")?, contribution, writer)?;
            assert!(
                writer
                    .record_with_origins(
                        &crate::Subresource::new("synthetic", Vec::new()),
                        crate::Status::Complete,
                        &serde_json::json!({"present": 1}),
                        &[],
                        missing
                    )
                    .is_err()
            );
            Ok(())
        },
    )
}

#[test]
fn equal_bytes_from_another_root_archive_do_not_acquire_the_active_root_authority() -> TestResult {
    support::with_fixture(
        super::entries(),
        Inventory::Intact,
        &Exclusion::default(),
        |context, writer, _, folder| {
            let path = folder.join("other.p4k");
            p4kx_testkit::ArchiveBuilder::new()
                .entry(super::entries().into_iter().next().ok_or("owner fixture")?)
                .entry(
                    super::entries()
                        .into_iter()
                        .nth(1)
                        .ok_or("unread fixture")?,
                )
                .write_to(&path)?;
            assert_eq!(
                std::fs::read(context.directory.index.archive.path())?,
                std::fs::read(&path)?
            );
            let foreign = p4kx_extraction::Archive::open(path)?;
            let entry = foreign.entries().first().ok_or("foreign owner Entry")?;
            let before = writer.origins.owner.freeze()?.sources().len();
            assert!(
                writer.origins.input(&foreign, entry).is_err(),
                "matching bytes, paths and physical ordinals do not prove original root ownership"
            );
            assert_eq!(
                writer.origins.owner.freeze()?.sources().len(),
                before,
                "a foreign root cannot add source rows"
            );
            Ok(())
        },
    )
}

#[test]
fn source_context_refuses_another_real_entry_and_another_payload_with_the_same_record_identity()
-> TestResult {
    support::with_fixture(
        super::entries(),
        Inventory::Intact,
        &Exclusion::default(),
        |context, writer, _, folder| {
            let genuine = context.origin_input(writer)?;
            assert!(genuine.owner.is_same_inventory(&writer.origins.owner));
            let another = context
                .directory
                .index
                .archive
                .entries()
                .get(1)
                .ok_or("second Entry")?;
            let mismatched = crate::SourceContext {
                source: context.source,
                entry: another,
                payload: context.payload,
                directory: context.directory,
            };
            assert!(
                mismatched.origin_input(writer).is_err(),
                "an admitted entry still must match the active original subject"
            );
            let path = folder.join("other-payload");
            std::fs::copy(context.payload, &path)?;
            let displaced = crate::SourceContext {
                source: context.source,
                entry: context.entry,
                payload: &path,
                directory: context.directory,
            };
            assert!(
                displaced.origin_input(writer).is_err(),
                "matching bytes at another path do not replace the active retained payload"
            );
            Ok(())
        },
    )
}
