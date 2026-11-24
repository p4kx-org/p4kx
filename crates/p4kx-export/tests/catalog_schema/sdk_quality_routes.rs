// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection,
};
use p4kx_extraction::Archive;
use p4kx_testkit::{ArchiveBuilder, DataForgeBuilder, TestEntry, TestRecord, TestStruct};

use super::super::{QUALITY_DISTRIBUTIONS, QUALITY_QUANTIZATION_RESOURCES, QUALITY_QUANTIZATIONS};
use super::{Route, TestResult};

const DISTRIBUTION: &str = "CraftingQualityDistributionRecord";
const QUANTIZATION: &str = "CraftingQualityQuantizationRecord";
const REMAINING: &str =
    "typed selection and synthetic producer parity available; installed producer parity open";

pub(super) fn read() -> TestResult<[Route; 3]> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("Data.p4k");
    let data = DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new(DISTRIBUTION))
        .structure(TestStruct::new(QUANTIZATION))
        .record(TestRecord::new(
            1,
            DISTRIBUTION,
            "CraftingQualityDistributionRecord.Fixture",
            "libs/foundry/records/crafting/qualitydistribution/fixture.xml",
        ))
        .record(TestRecord::new(
            2,
            QUANTIZATION,
            "CraftingQualityQuantizationRecord.Fixture",
            "libs/foundry/records/crafting/qualityquantization/fixture.xml",
        ));
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", data.build()))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let session =
        CatalogQuerySession::open(&archive, CatalogLanguageSelection::Selected(&[]), None)?;
    let parsed = session.parsed(None)?;
    Ok([
        parsed.with_quality_distributions(
            CatalogRecordSelection::RecordIndex(0),
            None,
            |snapshot| Route::queried_rows(QUALITY_DISTRIBUTIONS, snapshot.rows(), REMAINING),
        )??,
        parsed.with_quality_quantizations(
            CatalogRecordSelection::RecordIndex(1),
            None,
            |snapshot| Route::queried_rows(QUALITY_QUANTIZATIONS, snapshot.rows(), REMAINING),
        )??,
        parsed.with_quality_quantization_resources(
            CatalogRecordSelection::RecordIndex(1),
            None,
            |snapshot| {
                Route::queried_rows(
                    QUALITY_QUANTIZATION_RESOURCES,
                    snapshot.projection.rows(),
                    None,
                )
            },
        )??,
    ])
}
