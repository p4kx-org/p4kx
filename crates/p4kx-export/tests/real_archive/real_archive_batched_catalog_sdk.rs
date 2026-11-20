// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Archive;

use crate::real_archive_cargo_globals::complete_catalog as cargo;
use crate::real_archive_catalog_table_parity::{Origins, Table, TestResult, verify_tables};
use crate::real_archive_law_sdk as law;
use crate::real_archive_quality_inverse_sdk as quality;
use crate::real_archive_verification::print_line;

#[test]
#[ignore = "same Build: set P4KX_DATA_P4K, P4KX_LAW_SDK_BUILD and P4KX_CATALOG_MEASUREMENT_DIR"]
fn installed_law_quality_and_cargo_sdk_match_one_complete_catalog() -> TestResult {
    let [infractions, sets, jurisdictions, manifests] = law::table_paths();
    let [loading, prices] = cargo::TABLE_PATHS;
    let counts = verify_tables(
        [
            infractions,
            sets,
            jurisdictions,
            manifests,
            quality::QUANTIZATION_RESOURCES,
            loading,
            prices,
        ],
        read_sdk,
    )?;
    let [
        infractions,
        sets,
        jurisdictions,
        manifests,
        quantizers,
        loading,
        prices,
    ] = counts.rows;
    print_line(&format!(
        "complete Catalog SDK parity: lawRows=[{infractions}, {sets}, {jurisdictions}, {manifests}] qualityInverseRows={quantizers} cargoRows=[{loading}, {prices}] sourceUses={} physicalSources={}",
        counts.source_uses, counts.physical_sources,
    ));
    Ok(())
}

fn read_sdk(archive: &Archive, origins: &mut Origins) -> TestResult<[Table; 7]> {
    let [infractions, sets, jurisdictions, manifests] = law::capture_sdk(archive, origins)
        .map_err(|error| format!("law SDK preflight: {error}"))?;
    let [quantizers] = quality::read_sdk(archive, origins)
        .map_err(|error| format!("quality inverse SDK preflight: {error}"))?;
    let [loading, prices] = cargo::read_sdk(archive, origins)
        .map_err(|error| format!("cargo SDK preflight: {error}"))?;
    Ok([
        infractions,
        sets,
        jurisdictions,
        manifests,
        quantizers,
        loading,
        prices,
    ])
}
