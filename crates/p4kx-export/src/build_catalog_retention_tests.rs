// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;
use std::time::UNIX_EPOCH;

use p4kx_extraction::{Archive, DataForgeEntry};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestRecord, TestStruct, TestType, TestValue,
};
use rayon::ThreadPoolBuilder;

use super::super::write_catalog_from_with_run;
use super::support::TestResult;
use crate::export_field_inspection::FieldInspection;
use crate::options::{CatalogRetention, CatalogRun, CatalogStage};
use crate::{CatalogOptions, Compression};

#[test]
fn retention_witness_preserves_the_compressed_catalog_and_phase_sequence() -> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = fixture(folder.path())?;
    let entry = DataForgeEntry::read(&archive)?;
    let options = CatalogOptions::new(UNIX_EPOCH);
    let one = capture(&archive, &entry, &options, 1)?;
    let two = capture(&archive, &entry, &options, 2)?;
    assert_eq!(
        one.output, two.output,
        "the complete compressed wire output and references agree across thread counts"
    );
    assert_eq!(
        one.phases, two.phases,
        "the normal phase sequence agrees across thread counts"
    );

    let retained = two.retained;
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
        after_records.packed_source_uses > before_records.packed_source_uses,
        "draining the record window packs unheld uses before projections"
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

struct Capture {
    output: Vec<u8>,
    phases: Vec<CatalogStage>,
    retained: Vec<CatalogRetention>,
}

fn capture(
    archive: &Archive,
    entry: &DataForgeEntry,
    options: &CatalogOptions<'_>,
    threads: usize,
) -> TestResult<Capture> {
    let pool = ThreadPoolBuilder::new().num_threads(threads).build()?;
    let captured = pool.install(|| {
        let phases = RefCell::new(Vec::new());
        let phase_report = |stage| phases.borrow_mut().push(stage);
        let retained = RefCell::new(Vec::new());
        let retention_report = |sample| retained.borrow_mut().push(sample);
        let run = CatalogRun::new(options, Some(&phase_report)).with_retention(&retention_report);
        let (_, output) =
            write_catalog_from_with_run(archive, entry, run, Compression::ZSTD, || Ok(Vec::new()))
                .map_err(|error| error.to_string())?;
        Ok::<_, String>(Capture {
            output,
            phases: phases.into_inner(),
            retained: retained.into_inner(),
        })
    })?;
    Ok(captured)
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
