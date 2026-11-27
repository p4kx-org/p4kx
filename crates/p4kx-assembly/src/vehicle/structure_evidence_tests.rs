// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_archive::{Archive, SourceEncryption};
use p4kx_extraction::SourceLocation;

use super::support::{self, TestResult};
use p4kx_dataforge::{DataForge, TreeBuilder};
use p4kx_testkit::{ArchiveBuilder, DataForgeBuilder, TestEntity, TestEntry};

use crate::{
    ArchiveFiles, LoadoutDocumentKind, LoadoutError, LoadoutFileError, LoadoutFiles,
    VehicleStructureEvidence, read_observed_vehicle_tree,
};

const BASE: &[u8] = br#"<Vehicle><Parts><Part id="target" mass="2"/></Parts><Modifications>
    <Modification name="observed"><Elems><Elem idRef="target" name="behavior" value="wrong"/></Elems></Modification>
    <Modification name="Observed"><Elems>
        <Elem idRef="target" name="behavior" value="applied"/>
        <Elem idRef="target" name="flag"/>
        <Elem idRef="missing" name="behavior" value="unmatched"/>
        <Elem idRef="target" name="material" value="cosmetic"/>
    </Elems></Modification></Modifications></Vehicle>"#;
const PATCH: &[u8] = br#"<Patch><Part id="target" mass="5"/></Patch>"#;

#[test]
fn digest_retains_actual_document_operations_and_applied_elem_values() -> TestResult {
    support::with_vehicle(BASE, Some(PATCH), "Observed", true, |vehicle, observer| {
        let evidence = vehicle
            .structure_evidence()
            .ok_or("digest source evidence")?;
        assert!(
            evidence.is_complete(),
            "the structural comparison inputs close"
        );
        check_operations(&evidence, observer)?;
        let values: Vec<_> = evidence
            .selectors()
            .filter_map(|site| support::base_fragment(site, BASE))
            .filter(|fragment| fragment.starts_with("value"))
            .collect();
        assert_eq!(
            values,
            ["value=\"applied\""],
            "only the applied compared Elem supplies a value"
        );
        assert!(
            evidence.selectors().any(|site| {
                site.checked_absence()
                    && matches!(site.location(), Some(SourceLocation::Subresource { field, .. }) if field == "value")
            }),
            "an applied omitted value retains the exact node where the default was observed"
        );
        assert_eq!(
            vehicle.totals().mass(),
            5.0,
            "the mass result retains its existing input"
        );
        Ok(())
    })
}

fn check_operations(
    evidence: &VehicleStructureEvidence<'_>,
    observer: &support::Observer,
) -> TestResult {
    let parsed = observer
        .parsed
        .lock()
        .map_err(|_| "original parse witnesses")?;
    assert_eq!(parsed.len(), 2, "only the base and patch are projected");
    let mut documents = evidence.applied_documents();
    let base = documents.next().ok_or("resolved base operation")?;
    let patch = documents.next().ok_or("applied patch operation")?;
    assert_eq!(
        base.kind(),
        LoadoutDocumentKind::VehicleImplementation,
        "the base is first"
    );
    assert_eq!(
        patch.kind(),
        LoadoutDocumentKind::VehiclePatch,
        "the applied patch is second"
    );
    assert!(documents.next().is_none(), "unused inputs are excluded");
    for document in [base, patch] {
        assert!(
            parsed
                .iter()
                .any(|original| original.is_same_operation(document)),
            "the proof retains the original parse operation"
        );
    }
    drop(parsed);
    assert_eq!(
        patch
            .source()
            .entry_origin()
            .ok_or("the patch Entry")?
            .encryption(),
        SourceEncryption::Encrypted,
        "a structural patch preserves its actual protected payload"
    );
    Ok(())
}

#[test]
fn an_unreadable_selected_patch_does_not_become_an_applied_document() -> TestResult {
    support::with_vehicle(
        BASE,
        Some(b"<Patch>"),
        "Observed",
        true,
        |vehicle, observer| {
            let evidence = vehicle
                .structure_evidence()
                .ok_or("digest source evidence")?;
            assert!(
                !evidence.is_complete(),
                "the failed selected patch prevents comparison"
            );
            assert!(
                vehicle.complete_structure_digest().is_err(),
                "the digest remains unqualified"
            );
            assert_eq!(
                evidence.applied_documents().count(),
                1,
                "only the valid base was committed"
            );
            let parsed = observer
                .parsed
                .lock()
                .map_err(|_| "original parse witnesses")?;
            assert!(
                parsed.iter().any(|file| {
                    file.kind() == LoadoutDocumentKind::VehiclePatch
                        && matches!(file.projection_outcome(), Some(Err(_)))
                }),
                "the failed original parse remains visible through its separate observation"
            );
            drop(parsed);
            assert_eq!(
                vehicle.totals().mass(),
                2.0,
                "the mass fallback still uses the base"
            );
            Ok(())
        },
    )
}

#[test]
fn unconfigured_and_missing_base_have_different_original_read_outcomes() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Data.p4k");
    ArchiveBuilder::v2()
        .entry(TestEntry::stored(
            "Data/unrelated.xml",
            b"<Unused/>".as_slice(),
        ))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let files = ArchiveFiles::new(&archive);
    let input = p4kx_testkit::entity_schema(DataForgeBuilder::new())
        .record(TestEntity::new(71, "Unconfigured", "synthetic/unconfigured.xml").record())
        .record(
            TestEntity::new(72, "Configured", "synthetic/configured.xml")
                .vehicle(support::BASE, "")
                .record(),
        )
        .build();
    let forge = DataForge::open(&input)?;
    let observer = std::sync::Arc::new(support::Observer::new(true));
    let unconfigured = forge.record(0).ok_or("the unconfigured physical owner")?;
    let unconfigured_tree = TreeBuilder::new().build(&unconfigured)?;
    assert!(
        read_observed_vehicle_tree(
            &forge,
            &unconfigured,
            &unconfigured_tree,
            &files,
            observer.clone()
        )?
        .is_none(),
        "an owner without a Vehicle component has no configured lookup"
    );
    assert!(
        observer
            .read_paths
            .lock()
            .map_err(|_| "file-read witness")?
            .is_empty(),
        "an unconfigured owner never searches the archive"
    );
    check_missing_base(&forge, &files, observer)
}

fn check_missing_base(
    forge: &DataForge<'_>,
    files: &ArchiveFiles<'_>,
    observer: std::sync::Arc<support::Observer>,
) -> TestResult {
    let configured = forge.record(1).ok_or("the configured physical owner")?;
    let configured_tree = TreeBuilder::new().build(&configured)?;
    let Err(LoadoutError::MissingVehicleImplementation { path }) =
        read_observed_vehicle_tree(forge, &configured, &configured_tree, files, observer)
    else {
        return Err("a configured owner has no implementation only after lookup".into());
    };
    assert!(
        path.caller().is_some(),
        "the original caller binds the missing candidates"
    );
    assert!(
        !path.attempts().is_empty(),
        "the absence contains actual file attempts"
    );
    assert!(
        path.attempts().iter().all(|attempt| {
            attempt.kind() == LoadoutDocumentKind::VehicleImplementation
                && matches!(attempt.read_outcome(), Err(LoadoutFileError::NotFound))
                && attempt.source().is_missing_archive()
        }),
        "all tried base candidates were absent from the original archive"
    );
    Ok(())
}

#[test]
fn verified_vehicle_read_keeps_the_failed_original_entry_admission() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Data.p4k");
    let damaged_path = "Data/damaged.xml";
    ArchiveBuilder::v2()
        .entry(TestEntry::stored("Data/verified.xml", BASE))
        .entry(TestEntry::stored(damaged_path, BASE).with_crc32c(0))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let verified = ArchiveFiles::new(&archive).with_verified_entries();
    assert_eq!(
        verified.read("verified.xml")?,
        BASE,
        "verified mode reads the original complete XML"
    );
    let unverified = ArchiveFiles::new(&archive);
    assert_eq!(
        unverified.read("damaged.xml")?,
        BASE,
        "the decoded document is readable without verification"
    );
    let failed = verified.read_observed("damaged.xml");
    assert!(
        matches!(failed.outcome(), Err(LoadoutFileError::Unreadable { .. })),
        "integrity failure cannot provide a complete Vehicle input"
    );
    let entry = archive
        .entries()
        .iter()
        .find(|entry| entry.path().replace('\\', "/") == damaged_path)
        .ok_or("the exact damaged original Entry")?;
    let expected = archive
        .entry_origin(entry)
        .ok_or("the original admission")?;
    let observed = failed
        .source()
        .entry_origin()
        .ok_or("the failed admission")?;
    assert_eq!(
        observed.binding(),
        expected.binding(),
        "the attempted read keeps the same physical original Entry"
    );
    Ok(())
}
