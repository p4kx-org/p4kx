// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogRecordSelection, DatasetAvailability, ResourceNumber,
};
use p4kx_export::{CatalogOptions, CatalogPublicationMode};
use p4kx_extraction::SourceValueState;
use serde_json::{Value, json};

use super::{DcbOrigin, Layout, ROWS, TestResult, common, database, proof};
use crate::sdk_full_proof as full_proof;

#[test]
fn shield_sdk_matches_canonical_rows_with_original_states_and_provenance() -> TestResult {
    for (mode, encryption) in [(DcbOrigin::Plain, false), (DcbOrigin::Encrypted, true)] {
        let (_folder, archive) = common::open(&database(Layout::Full), mode)?;
        let catalog: Value = serde_json::from_slice(
            p4kx_export::build_catalog(&archive, common::options().with_full_provenance())?.json(),
        )?;
        full_proof::open(&archive, full_proof::NO_LANGUAGES, None)?.with_shield_generators(
            CatalogRecordSelection::All,
            None,
            |snapshot| -> TestResult {
                let [row] = snapshot.rows() else {
                    return Err("one canonical physical shield generator".into());
                };
                assert_eq!(snapshot.coverage().incomplete_fields, [] as [u32; 0]);
                assert_eq!(snapshot.requested_languages(), []);
                assert_eq!(row.owner().record_index().value(), Some(&0));
                assert_eq!(row.owner().component_ordinal().value(), Some(&1));
                let Some(ResourceNumber::Single(health)) = row.max_health().value() else {
                    return Err("original Float32 shield health".into());
                };
                assert_eq!(health.raw_bits(), 2048.0_f32.to_bits());
                assert_eq!(
                    row.max_health().origin().contains_encrypted_source(),
                    Some(encryption),
                );
                let slots = row
                    .shield_resistance()
                    .value()
                    .ok_or("original resistance array")?;
                assert_eq!(slots.len(), 7, "every original slot is retained");
                assert!(!row.additional_fields().roots().is_empty());
                let [expected] = proof::rows(&catalog, ROWS)? else {
                    return Err("one canonical shield generator row".into());
                };
                let sdk = json!({"provenance": snapshot.source_inventory()});
                assert_eq!(
                    proof::with_origins(serde_json::from_slice(&serde_json::to_vec(row)?)?, &sdk)?,
                    proof::with_origins(expected.clone(), &catalog)?,
                    "all emitted cells and physical sources match the canonical row",
                );
                Ok(())
            },
        )??;
    }
    Ok(())
}

#[test]
fn shield_sdk_keeps_a_wrong_storage_cell_and_reports_owner_coverage() -> TestResult {
    let (_folder, archive) = common::open(&database(Layout::Invalid), DcbOrigin::Encrypted)?;
    let options = CatalogOptions {
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..common::options()
    };
    let catalog: Value = serde_json::from_slice(
        p4kx_export::build_catalog(&archive, options.with_full_provenance())?.json(),
    )?;
    let session = full_proof::open(&archive, CatalogLanguageSelection::AllAvailable, None)?;
    let parsed = session.parsed(None)?;
    parsed.with_shield_generators(
        CatalogRecordSelection::RecordIndex(1),
        None,
        |snapshot| -> TestResult {
            assert!(
                snapshot.rows().is_empty(),
                "this owner has no shield component"
            );
            assert!(matches!(
                snapshot.availability(),
                DatasetAvailability::Available
            ));
            Ok(())
        },
    )??;
    parsed.with_shield_generators(
        CatalogRecordSelection::RecordIndex(0),
        None,
        |snapshot| -> TestResult {
            let [row] = snapshot.rows() else {
                return Err("one original partial shield row".into());
            };
            assert_eq!(snapshot.coverage().incomplete_fields, [0]);
            assert_eq!(row.shield_resistance().state(), SourceValueState::Invalid);
            let sdk = json!({"provenance": snapshot.source_inventory()});
            assert_eq!(
                proof::with_origins(serde_json::from_slice(&serde_json::to_vec(row)?)?, &sdk)?,
                proof::with_origins(proof::at(&catalog, &format!("{ROWS}/0"))?.clone(), &catalog)?,
                "partial row and its retained original field are identical",
            );
            Ok(())
        },
    )??;
    Ok(())
}
