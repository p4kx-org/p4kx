// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::equipment::{self, ShipSource};
use p4kx_export::catalog_queries::{CatalogLanguageSelection, CatalogRecordSelection};
use p4kx_export::{CatalogPublicationMode, build_catalog};
use p4kx_extraction::DataForgeEntry;
use serde_json::Value;

use super::fixture;
use crate::catalog_ship_base::source_sdk_walk;
use crate::commodity_v3_fixture::{self as common, TestResult};
use crate::health_origin_assertions as proof;

#[test]
fn direct_ship_sdk_nested_insurance_getters_keep_both_original_physical_owners() -> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = fixture::open(&folder.path().join("Data.p4k"))?;
    let entry = DataForgeEntry::read(&archive)?;
    let mut options = common::options();
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let catalog: Value = serde_json::from_slice(build_catalog(&archive, &options)?.json())?;
    equipment::with_ship_sources_from(
        &archive,
        &entry,
        CatalogRecordSelection::All,
        CatalogLanguageSelection::Selected(&[]),
        None,
        |snapshot| {
            assert_eq!(
                snapshot.rows().len(),
                2,
                "both same-GUID original records survive"
            );
            for ((row, case), canonical) in snapshot
                .rows()
                .iter()
                .zip(&fixture::CASES)
                .zip(proof::rows(&catalog, "/ships")?)
            {
                source_sdk_walk::row(row, snapshot.source_inventory(), canonical)?;
                original_values(row, case)?;
            }
            Ok::<_, Box<dyn std::error::Error>>(())
        },
    )??;
    proof::closure(&catalog)
}

fn original_values(row: &ShipSource<'_>, case: &fixture::Case) -> TestResult {
    assert_eq!(
        row.source().class_name().value(),
        Some(&case.class),
        "selected original class remains physical"
    );
    assert_eq!(
        row.vehicle_definition().value(),
        Some(&case.definition),
        "blank and whitespace original selectors remain distinct"
    );
    assert_eq!(
        row.source().guid().origin().contains_encrypted_source(),
        Some(true),
        "the original DCB is genuinely encrypted"
    );
    let insurance = row
        .insurance()
        .value()
        .ok_or("positive original insurance")?;
    assert_eq!(
        insurance.restore_on_claim().value(),
        Some(&case.restore),
        "the two physical insurance declarations stay distinct"
    );
    assert_eq!(
        insurance
            .params()
            .value()
            .ok_or("positive original insurance parameters")?
            .ship_entity_class_name()
            .value(),
        Some(&case.insured_class),
        "original empty class name remains present"
    );
    Ok(())
}
