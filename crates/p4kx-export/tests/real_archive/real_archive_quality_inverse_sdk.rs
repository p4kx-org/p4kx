// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::CatalogLanguageSelection;
use p4kx_export::catalog_queries::{CatalogRecordSelection, DatasetAvailability};
use p4kx_extraction::Archive;
use serde_json::Value;

use crate::real_archive_catalog_table_parity::{self as parity, Origins, Table, TestResult};
use crate::real_archive_scoped_equipment_sdk::equipment_input::full_source_session;
use crate::real_archive_verification;

pub(super) const QUANTIZATION_RESOURCES: &str = "/projections/qualityQuantizationResources";

#[test]
#[ignore = "same Build: set P4KX_DATA_P4K and P4KX_CATALOG_MEASUREMENT_DIR"]
fn installed_quality_inverse_sdk_matches_the_same_complete_catalog() -> TestResult {
    let counts = parity::verify_tables([QUANTIZATION_RESOURCES], read_sdk)?;
    let [quantizers] = counts.rows;
    real_archive_verification::print_line(&format!(
        "quality inverse SDK parity: quantizers={quantizers} sourceUses={} physicalSources={}",
        counts.source_uses, counts.physical_sources,
    ));
    Ok(())
}

pub(super) fn read_sdk(archive: &Archive, origins: &mut Origins) -> TestResult<[Table; 1]> {
    let session = full_source_session(archive, CatalogLanguageSelection::Selected(&[]))?;
    session.with_quality_quantization_resources(CatalogRecordSelection::All, None, |snapshot| {
        let projection = &snapshot.projection;
        if projection.build() != archive.build()
            || !matches!(projection.availability(), DatasetAvailability::Available)
            || projection.rows().is_empty()
        {
            return Err(
                "the inverse query requires quantizers from the matching Archive Build".into(),
            );
        }
        for (scope, coverage) in [
            ("quantizer", projection.coverage()),
            ("dependency", &snapshot.dependency_coverage),
        ] {
            if !coverage.unreadable_headers.is_empty()
                || !coverage.unreadable_bodies.is_empty()
                || !coverage.incomplete_fields.is_empty()
            {
                return Err(format!("incomplete quality inverse {scope} coverage").into());
            }
        }

        Ok([parity::capture_rows(
            projection.availability(),
            projection.rows(),
            projection.source_inventory(),
            origins,
            physical_record_index,
        )?])
    })?
}

fn physical_record_index(row: &Value) -> TestResult<u32> {
    let index = row
        .pointer("/recordIndex/value")
        .and_then(Value::as_u64)
        .ok_or("required physical quantizer Record index")?;
    Ok(u32::try_from(index)?)
}
