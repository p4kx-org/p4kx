// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;
use std::time::UNIX_EPOCH;

use p4kx_extraction::{Archive, DataForgeEntry};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestRecord, TestStruct, TestType, TestValue,
};

use super::super::write_catalog_from_with_run;
use super::support::TestResult;
use crate::export_field_inspection::FieldInspection;
use crate::options::{CatalogRun, CatalogStage};
use crate::{CatalogOptions, Compression};

#[test]
fn retention_witness_preserves_the_compressed_catalog_and_phase_sequence() -> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = fixture(folder.path())?;
    let entry = DataForgeEntry::read(&archive)?;
    let options = CatalogOptions::new(UNIX_EPOCH);
    let regular_phases = RefCell::new(Vec::new());
    let regular_report = |stage| regular_phases.borrow_mut().push(stage);
    let regular_run = CatalogRun::new(&options, Some(&regular_report));
    let (_, regular) =
        write_catalog_from_with_run(&archive, &entry, regular_run, Compression::ZSTD, || {
            Ok(Vec::new())
        })?;

    let observed_phases = RefCell::new(Vec::new());
    let phase_report = |stage| observed_phases.borrow_mut().push(stage);
    let retained = RefCell::new(Vec::new());
    let retention_report = |sample| retained.borrow_mut().push(sample);
    let observed_run =
        CatalogRun::new(&options, Some(&phase_report)).with_retention(&retention_report);
    let (_, observed) =
        write_catalog_from_with_run(&archive, &entry, observed_run, Compression::ZSTD, || {
            Ok(Vec::new())
        })?;
    assert_eq!(
        regular, observed,
        "the complete compressed wire output agrees"
    );
    assert_eq!(
        regular_phases, observed_phases,
        "the normal phase sequence agrees"
    );

    let retained = retained.into_inner();
    let [before_records, after_records, before_health] = retained.as_slice() else {
        return Err("expected exactly three retention measurements".into());
    };
    assert_eq!(before_records.stage, CatalogStage::Records);
    assert_eq!(after_records.stage, CatalogStage::Projections);
    assert_eq!(before_health.stage, CatalogStage::ExportHealth);
    assert!(
        after_records.source_uses > before_records.source_uses,
        "the selected record registers proof"
    );
    assert!(
        after_records.field_inspections > 0,
        "the selected record leaves a field inspection"
    );
    for sample in &retained {
        assert!(sample.inspection_capacity >= sample.field_inspections);
        assert_eq!(
            sample.inline_inspection_bytes,
            sample.inspection_capacity * size_of::<FieldInspection>(),
            "reserved inline bytes match the live vector's capacity"
        );
    }
    Ok(())
}

fn fixture(folder: &std::path::Path) -> TestResult<Archive> {
    let database = DataForgeBuilder::new()
        .structure(TestStruct::new("SCItemManufacturer").field("Code", TestType::String))
        .record(
            TestRecord::new(
                1,
                "SCItemManufacturer",
                "SCItemManufacturer.Fixture",
                "libs/foundry/records/scitemmanufacturer/fixture.xml",
            )
            .value("Code", TestValue::text("FIXTURE")),
        )
        .build();
    let path = folder.join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", database))
        .write_to(&path)?;
    Ok(Archive::open(path)?)
}
