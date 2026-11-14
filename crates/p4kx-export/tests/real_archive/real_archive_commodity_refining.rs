// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The density and the refined version of the commodities of the Basetool catalog of a real
//! Archive; runs with `--ignored` when `P4KX_DATA_P4K` names one. Only a density in
//! `GramsPerCubicCentimeter` is read, so a commodity whose `ResourceType` uses another unit
//! subclass counts as without a density.

use std::collections::HashSet;
use std::error::Error;
use std::path::Path;
use std::time::SystemTime;

use p4kx_export::{CatalogOptions, build_catalog};
use p4kx_extraction::Archive;
use p4kx_testkit::real_archive_path;
use serde_json::Value;

use crate::catalog_v3_consumer;

type TestResult = Result<(), Box<dyn Error>>;

#[expect(
    clippy::print_stdout,
    reason = "the measurements are the result of the verification"
)]
fn print_line(line: &str) {
    println!("{line}");
}

fn verify(path: &Path) -> TestResult {
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::real_archive_key()?))?;
    let built = build_catalog(&archive, &CatalogOptions::new(SystemTime::now()))?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let commodities = catalog_v3_consumer::presentations(&catalog)?;
    let Measurements {
        subtypes,
        densities,
        refined,
        own,
    } = measurements(commodities)?;
    let dangling = refined
        .iter()
        .filter(|guid| !subtypes.contains(**guid))
        .count();
    let lowest = densities.iter().copied().fold(f64::INFINITY, f64::min);
    let highest = densities.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    print_line(&format!(
        "{}: {} commodities, {} with a density ({lowest} to {highest} g/cm3), {} with a refined version, {dangling} of those without a commodity, {own} refined into themselves",
        path.display(),
        commodities.len(),
        densities.len(),
        refined.len(),
    ));
    assert!(!densities.is_empty(), "some commodity has a density");
    assert!(
        densities
            .iter()
            .all(|density| *density > 0.0 && *density < 100.0),
        "every density is positive and plausible"
    );
    assert!(!refined.is_empty(), "some commodity has a refined version");
    assert_eq!(own, 0, "no commodity is refined into itself");
    Ok(())
}

struct Measurements<'catalog> {
    subtypes: HashSet<&'catalog str>,
    densities: Vec<f64>,
    refined: Vec<&'catalog str>,
    own: usize,
}

fn measurements(commodities: &[Value]) -> Result<Measurements<'_>, Box<dyn Error>> {
    let mut subtypes = HashSet::new();
    let mut densities = Vec::new();
    let mut refined = Vec::new();
    let mut own = 0;
    for commodity in commodities {
        let subtype = catalog_v3_consumer::nullable_guid(
            commodity.get("subtypeGuid").ok_or("subtype carrier")?,
        )?;
        if let Some(guid) = subtype {
            subtypes.insert(guid);
        }
        let refinement = catalog_v3_consumer::nullable_guid(
            commodity
                .get("refinedSubtypeGuid")
                .ok_or("refinement carrier")?,
        )?;
        if let Some(guid) = refinement {
            refined.push(guid);
            if subtype == Some(guid) {
                own += 1;
            }
        }
        let density = catalog_v3_consumer::present(
            commodity
                .get("densityGramsPerCubicCentimetre")
                .ok_or("density carrier")?,
        )?;
        if !density.is_null() {
            densities.push(density.as_f64().ok_or("nullable finite density")?);
        }
    }
    Ok(Measurements {
        subtypes,
        densities,
        refined,
        own,
    })
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn the_density_and_refined_version_of_a_real_archive_are_plausible() -> TestResult {
    verify(&real_archive_path()?)
}
