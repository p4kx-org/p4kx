// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceLocation, SourceUseRole};
use p4kx_testkit::{DataForgeBuilder, TestEntity, TestEntry};

use super::{Capture, Decision, Operands, VehicleStructure, read};
use crate::Kind;
use crate::build::original_input::ship_pool_test_input;
use crate::build::original_input::test_support::TestResult;
use crate::catalog_v3::authored_fields::ship_source_read;
use crate::export_diagnostic::ExportSourceRole;
use crate::record_observation::RecordObservation;

const BASE: &str = "Scripts/Entities/Vehicles/Implementations/Xml/ScopedBinding.xml";
const PATCH: &str =
    "Scripts/Entities/Vehicles/Implementations/Xml/Modifications/ScopedBinding_Selected.xml";
const DOCUMENT: &[u8] = br#"<Vehicle><Parts><Part id="root" mass="2"/></Parts>
    <Modifications><Modification name="Selected"><Elems>
        <Elem idRef="root" name="behavior" value="bound"/>
    </Elems></Modification></Modifications>
</Vehicle>"#;
const PATCH_BODY: &[u8] = br#"<Patch><Part id="root" mass="3"/></Patch>"#;

#[test]
fn digest_binds_the_selected_encrypted_patch_and_its_actual_elem() -> TestResult {
    let entries = vec![
        TestEntry::stored(format!("Data/{BASE}"), DOCUMENT),
        TestEntry::stored(format!("Data/{PATCH}"), PATCH_BODY).encrypted(),
        TestEntry::stored("Data/unrelated.xml", b"<Vehicle/>".as_slice()).encrypted(),
    ];
    let database = configured_database("Selected");
    with_structure(&database, entries, |result, uses| {
        assert!(
            matches!(result.decision, Decision::Digest(_)),
            "the complete digest is comparable"
        );
        let origin = &result.evidence.origin;
        assert!(
            origin.source_set_complete(),
            "the same-caller digest proof closes"
        );
        assert_eq!(
            origin.contains_encrypted_source(),
            Some(true),
            "the patch payload is protected"
        );
        assert_eq!(
            uses.iter().filter(|used| {
                origin.source_uses_refs().contains(used.id())
                    && used.role() == SourceUseRole::Conversion
                    && matches!(used.location(), SourceLocation::Subresource { namespace, field, .. } if namespace == "loadoutFileRead" && field == "content")
            }).count(),
            2,
            "only the committed base and patch contribute complete payloads"
        );
        assert!(
            uses.iter().any(|used| {
                origin.source_uses_refs().contains(used.id())
                    && matches!(used.location(), SourceLocation::Subresource { namespace, field, .. } if namespace == "textXmlAttribute" && field == "value")
            }),
            "the applied Elem has a precise original XML coordinate"
        );
        assert!(
            result.evidence.failed_source_uses.is_empty(),
            "no file operation failed"
        );
        Ok(())
    })
}

#[test]
fn empty_authored_modification_closes_the_base_digest() -> TestResult {
    let database = configured_database("");
    let entries = vec![TestEntry::stored(format!("Data/{BASE}"), DOCUMENT)];
    with_structure(&database, entries, |result, _uses| {
        assert!(
            matches!(result.decision, Decision::Digest(_)),
            "an empty modification selects the complete base document"
        );
        assert!(
            result.evidence.origin.source_set_complete(),
            "the original empty field proves no modification was needed"
        );
        Ok(())
    })
}

#[test]
fn proved_unconfigured_and_missing_base_have_distinct_states() -> TestResult {
    let unconfigured_database = p4kx_testkit::entity_schema(DataForgeBuilder::new())
        .record(TestEntity::new(47, "Unconfigured", "synthetic/unconfigured.xml").record())
        .build();
    with_structure(&unconfigured_database, Vec::new(), |result, _uses| {
        assert_eq!(
            result.decision,
            Decision::NoConfiguredVehicle,
            "the original owner has no configured XML"
        );
        assert!(
            result.evidence.origin.source_set_complete(),
            "the owner proves no configured XML"
        );
        Ok(())
    })?;
    let configured_database = configured_database("");
    with_structure(&configured_database, Vec::new(), |result, _uses| {
        assert_eq!(
            result.decision,
            Decision::MissingImplementation,
            "the complete directory has no selected implementation"
        );
        assert!(
            result.evidence.origin.source_set_complete(),
            "the original caller and directory close the lookup"
        );
        assert!(
            result.evidence.failed_source_uses.is_empty(),
            "no absent entry has an attempted payload"
        );
        Ok(())
    })
}

#[test]
fn verified_absent_explicit_patch_proves_no_comparison_digest() -> TestResult {
    const EXPLICIT_PATCH_DOCUMENT: &[u8] = br#"<Vehicle>
        <Parts><Part id="root" mass="2"/></Parts>
        <Modifications><Modification name="Selected" patchFile="RequiredExplicit.xml">
            <Elems><Elem idRef="root" name="behavior" value="bound"/></Elems>
        </Modification></Modifications>
    </Vehicle>"#;

    let database = configured_database("Selected");
    let entries = vec![TestEntry::stored(
        format!("Data/{BASE}"),
        EXPLICIT_PATCH_DOCUMENT,
    )];
    with_structure(&database, entries, |result, uses| {
        assert_eq!(
            result.decision,
            Decision::MissingExplicitPatch,
            "an absent required patch leaves the member outside structural comparison"
        );
        let origin = &result.evidence.origin;
        assert!(
            origin.source_set_complete(),
            "the exact original authored patch and complete directory prove ineligibility"
        );
        assert!(
            uses.iter().any(|used| {
                origin.source_uses_refs().contains(used.id())
                    && matches!(used.location(), SourceLocation::Subresource { namespace, field, .. } if namespace == "textXmlAttribute" && field == "patchFile")
            }),
            "the original explicit selector contributes to the negative decision"
        );
        assert!(
            uses.iter().any(|used| {
                origin.source_uses_refs().contains(used.id())
                    && matches!(
                        used.location(),
                        SourceLocation::ArchiveDirectoryPaths { .. }
                    )
            }),
            "the bound directory is original negative lookup evidence"
        );
        assert!(
            result.evidence.failed_source_uses.is_empty(),
            "the absent physical entry contributes no failed payload attempt"
        );
        Ok(())
    })
}

#[test]
fn checksum_failure_and_malformed_patch_do_not_become_comparable() -> TestResult {
    let bad_base = TestEntry::stored(format!("Data/{BASE}"), DOCUMENT).with_crc32c(0);
    let bad_patch = TestEntry::stored(format!("Data/{PATCH}"), b"<Patch>".as_slice());
    let base = TestEntry::stored(format!("Data/{BASE}"), DOCUMENT);
    for entries in [vec![bad_base], vec![base, bad_patch]] {
        let database = configured_database("Selected");
        with_structure(&database, entries, |result, uses| {
            assert_eq!(
                result.decision,
                Decision::Unavailable,
                "a failed physical document leaves the digest undecidable"
            );
            let origin = &result.evidence.origin;
            let attempted = &result.evidence.failed_source_uses;
            assert!(
                !origin.source_set_complete(),
                "the failed input cannot close the digest"
            );
            assert_eq!(
                attempted.len(),
                1,
                "the one actual failed payload is retained"
            );
            assert!(
                uses.iter().any(|used| {
                    used.role() == SourceUseRole::Attempted
                        && attempted.contains(used.id())
                        && !origin.source_uses_refs().contains(used.id())
                }),
                "failed payloads remain separate from contributions"
            );
            Ok(())
        })?;
    }
    Ok(())
}

fn configured_database(modification: &str) -> Vec<u8> {
    p4kx_testkit::entity_schema(DataForgeBuilder::new())
        .record(
            TestEntity::new(47, "ScopedBinding", "synthetic/scoped.xml")
                .vehicle(BASE, modification)
                .record(),
        )
        .build()
}

fn with_structure(
    database: &[u8],
    entries: Vec<TestEntry>,
    check: impl FnOnce(VehicleStructure, &[p4kx_extraction::SourceUseDescription]) -> TestResult,
) -> TestResult {
    ship_pool_test_input::with_input(database, entries, |archive, _, forge, input| {
        let record = forge
            .record(0)
            .ok_or("the original physical Vehicle owner")?;
        let tree = record.tree()?;
        let ledger = crate::export_field_walk::inspect(
            &tree,
            &record,
            Some(Kind::Loadouts),
            ExportSourceRole::Selected,
            None,
        )?;
        let operands = ship_source_read::aggregate_for_test(
            &tree,
            input,
            &RecordObservation::capture(input, &record)?,
        )?;
        let capture = std::sync::Arc::new(Capture::for_input(input, &ledger, None, Kind::Loadouts));
        let files = p4kx_assembly::ArchiveFiles::new(archive).with_verified_entries();
        let inputs = Operands {
            record_index: record.index(),
            vehicle: &operands.vehicle,
            modification: &operands.modification,
        };
        let result = read(&capture, &files, &record, &tree, inputs)?;
        let source_inventory = input
            .origins()
            .uses
            .finish_with(|| Ok::<(), crate::CatalogError>(()))?;
        check(result, source_inventory.uses())
    })
}
