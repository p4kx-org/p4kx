// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::{CatalogRecordSelection, ResourceNumber};
use p4kx_extraction::DataForgeEntry;
use serde_json::{Value, json};

use super::{DcbOrigin, Layout, ROWS, TestResult, common, fixture, proof};
use crate::sdk_full_proof as full_proof;

#[test]
fn quantum_sdk_keeps_physical_slots_and_all_bound_source_profiles() -> TestResult {
    for (mode, encryption) in [(DcbOrigin::Plain, false), (DcbOrigin::Encrypted, true)] {
        let (_folder, archive) = common::open(&fixture::database(Layout::Full), mode)?;
        let catalog: Value = serde_json::from_slice(
            p4kx_export::build_catalog(&archive, common::options().with_full_provenance())?.json(),
        )?;
        let entry = DataForgeEntry::read(&archive)?;
        let selection = CatalogRecordSelection::Guid(common::guid(fixture::OWNER).parse()?);
        full_proof::from_entry(&archive, &entry, full_proof::NO_LANGUAGES, None)?
            .with_quantum_drives(selection, None, |snapshot| -> TestResult {
                let rows = snapshot.rows();
                let canonical = proof::rows(&catalog, ROWS)?;
                assert_eq!(rows.len(), 2, "both selected original component slots");
                assert_eq!(rows.len(), canonical.len());
                assert_eq!(snapshot.coverage().incomplete_fields, [] as [u32; 0]);
                assert_eq!(snapshot.requested_languages(), []);
                let sdk = json!({"provenance": snapshot.source_inventory()});
                for ((row, expected), slot) in rows.iter().zip(canonical).zip(1..=2) {
                    assert_eq!(row.owner().record_index().value(), Some(&0));
                    assert_eq!(row.owner().component_ordinal().value(), Some(&slot));
                    let standard = row.standard().value().ok_or("original standard profile")?;
                    assert!(matches!(
                        standard.stage_one_accel_rate().value(),
                        Some(ResourceNumber::Single(_)),
                    ));
                    assert_eq!(
                        standard
                            .stage_one_accel_rate()
                            .origin()
                            .contains_encrypted_source(),
                        Some(encryption),
                    );
                    assert_eq!(
                        proof::with_origins(
                            serde_json::from_slice(&serde_json::to_vec(row)?)?,
                            &sdk
                        )?,
                        proof::with_origins(expected.clone(), &catalog)?,
                        "all authored nested profiles and physical origins match",
                    );
                }
                Ok(())
            })??;
    }
    Ok(())
}

#[test]
fn quantum_sdk_keeps_an_invalid_profile_and_reports_incomplete_coverage() -> TestResult {
    let (_folder, archive) = common::open(
        &fixture::database(Layout::InvalidSpline),
        DcbOrigin::Encrypted,
    )?;
    let mut options = common::options();
    options.publication_mode = p4kx_export::CatalogPublicationMode::DiagnosticPartial;
    let catalog: Value = serde_json::from_slice(
        p4kx_export::build_catalog(&archive, options.with_full_provenance())?.json(),
    )?;
    let entry = DataForgeEntry::read(&archive)?;
    full_proof::from_entry(&archive, &entry, full_proof::NO_LANGUAGES, None)?
        .with_quantum_drives(
            CatalogRecordSelection::RecordIndex(0),
            None,
            |snapshot| -> TestResult {
                let [row] = snapshot.rows() else {
                    return Err("one original quantum drive with an invalid spline slot".into());
                };
                assert_eq!(
                    snapshot.coverage().incomplete_fields,
                    [0],
                    "the malformed field is attributed to its physical owner",
                );
                let row = serde_json::from_slice(&serde_json::to_vec(row)?)?;
                proof::cell(&row, "spline", "invalid")?;
                let sdk = json!({"provenance": snapshot.source_inventory()});
                let [expected] = proof::rows(&catalog, ROWS)? else {
                    return Err("one diagnostic quantum drive row".into());
                };
                assert_eq!(
                    proof::with_origins(row, &sdk)?,
                    proof::with_origins(expected.clone(), &catalog)?,
                    "the diagnostic catalog and query retain the invalid cell and its sources",
                );
                Ok(())
            },
        )??;
    Ok(())
}
