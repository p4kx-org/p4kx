// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use p4kx_extraction::Archive;

use crate::real_archive_cargo_globals::complete_catalog as cargo;
use crate::real_archive_catalog_table_parity::{Origins, Table, TestResult, verify_tables};
use crate::real_archive_law_sdk as law;
use crate::real_archive_quality_inverse_sdk as quality;
use crate::real_archive_resource_configurations_sdk as resources;
use crate::real_archive_verification::print_line;

#[test]
#[ignore = "same Build: set P4KX_DATA_P4K, P4KX_LAW_SDK_BUILD and P4KX_CATALOG_MEASUREMENT_DIR"]
fn installed_law_quality_and_cargo_sdk_match_one_complete_catalog() -> TestResult {
    let counts = verify_tables(table_paths(), read_sdk)?;
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

#[test]
#[ignore = "same Build: set P4KX_DATA_P4K, P4KX_LAW_SDK_BUILD and P4KX_CATALOG_MEASUREMENT_DIR"]
fn installed_law_quality_cargo_and_resource_sdk_match_one_complete_catalog() -> TestResult {
    let law_paths = table_paths();
    let resource_paths = resources::TABLES;
    let families = [law_paths, resource_paths];
    let paths: [&str; 14] = families.as_flattened().try_into()?;
    if paths
        .iter()
        .enumerate()
        .any(|(at, path)| paths[..at].contains(path))
    {
        return Err("the combined Catalog comparison contains a duplicate dataset path".into());
    }

    let counts = verify_tables(paths, read_combined_sdk)?;
    let (law_rows, resource_rows) = counts.rows.split_at(law_paths.len());
    print_line(&format!(
        "complete Catalog SDK parity: lawQualityCargoRows={:?} resourceConfigurationRows={:?} combinedSourceUses={} combinedPhysicalSources={}",
        row_counts(&law_paths, law_rows),
        row_counts(&resource_paths, resource_rows),
        counts.source_uses,
        counts.physical_sources,
    ));
    Ok(())
}

fn table_paths() -> [&'static str; 7] {
    let [infractions, sets, jurisdictions, manifests] = law::table_paths();
    let [loading, prices] = cargo::TABLE_PATHS;
    [
        infractions,
        sets,
        jurisdictions,
        manifests,
        quality::QUANTIZATION_RESOURCES,
        loading,
        prices,
    ]
}

fn row_counts(paths: &[&'static str; 7], counts: &[usize]) -> BTreeMap<&'static str, usize> {
    paths.iter().copied().zip(counts.iter().copied()).collect()
}

fn read_combined_sdk(archive: &Archive, origins: &mut Origins) -> TestResult<[Table; 14]> {
    let [
        infractions,
        sets,
        jurisdictions,
        manifests,
        quantizers,
        loading,
        prices,
    ] = read_sdk(archive, origins)?;
    let [
        turrets,
        radar,
        globals,
        network,
        links,
        vehicle_rooms,
        item_rooms,
    ] = resources::read_sdk(archive, origins)
        .map_err(|error| format!("resource configuration SDK preflight: {error}"))?;
    Ok([
        infractions,
        sets,
        jurisdictions,
        manifests,
        quantizers,
        loading,
        prices,
        turrets,
        radar,
        globals,
        network,
        links,
        vehicle_rooms,
        item_rooms,
    ])
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
