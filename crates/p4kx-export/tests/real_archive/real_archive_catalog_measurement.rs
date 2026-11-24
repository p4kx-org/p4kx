// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::PathBuf;

use p4kx_extraction::{Archive, ArchiveWatch};

use crate::catalog_capture::CatalogCapture;
use crate::real_archive_catalog_preflight;
use crate::real_archive_catalog_size::replay::bindings::Receipt;

use super::compare_rows::Projection;
use super::{BoundMeasurement, TestResult};

pub(crate) fn with_bound_measurement<T>(
    verify: impl FnOnce(&Archive, &BoundMeasurement) -> TestResult<T>,
) -> TestResult<T> {
    with_measurement(None, verify)
}

pub(crate) fn with_bound_measurement_from_producer<T>(
    producer_revision: &str,
    verify: impl FnOnce(&Archive, &BoundMeasurement) -> TestResult<T>,
) -> TestResult<T> {
    with_measurement(Some(producer_revision), verify)
}

fn with_measurement<T>(
    producer_revision: Option<&str>,
    verify: impl FnOnce(&Archive, &BoundMeasurement) -> TestResult<T>,
) -> TestResult<T> {
    let directory = std::env::var_os("P4KX_CATALOG_MEASUREMENT_DIR")
        .map(PathBuf::from)
        .ok_or("configure the completed Catalog measurement")?;
    let receipt = Receipt::read(&directory)?;
    if let Some(producer_revision) = producer_revision {
        receipt.require_producer_revision(producer_revision)?;
    }
    let binding = receipt.binding()?;
    real_archive_catalog_preflight::check(&directory)?;
    let path = p4kx_testkit::real_archive_path().map_err(|_| "configure the matching Archive")?;
    let key = p4kx_testkit::real_archive_key().map_err(|_| "read the configured Archive key")?;
    let archive =
        Archive::open_with_key(path, Some(key)).map_err(|_| "open the matching Archive")?;
    let watch = ArchiveWatch::new(&archive);
    watch.check()?;
    binding.check_archive(&archive)?;

    let measurement = BoundMeasurement {
        directory,
        binding,
        watch,
    };
    let result = verify(&archive, &measurement);
    measurement.watch.check()?;
    result
}

pub(super) fn normal_capture<'scope>() -> CatalogCapture<Projection<'scope>> {
    let mut capture = CatalogCapture::default();
    for path in [
        "/schemaVersion",
        "/source",
        "/exportHealth/status",
        "/exportHealth/read",
        "/exportHealth/projection",
        "/exportHealth/publicationMode",
    ] {
        capture.keep(path);
    }
    capture.project("/completeness", Projection::CompleteSelection);
    capture
}
