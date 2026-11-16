// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{LoadoutReadEvent, LoadoutReadObserver};
use p4kx_extraction::DataForge;
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct};

use crate::export_diagnostic::ExportSourceRole;
use crate::{ExportAccountingError, Kind};

use super::{Capture, ExportLedger, State};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn source() -> Vec<u8> {
    DataForgeBuilder::new()
        .structure(TestStruct::new("EntityClassDefinition"))
        .record(TestRecord::new(
            0x6c00_0000_0000_4000_8000_0000_0000_0001,
            "EntityClassDefinition",
            "EntityClassDefinition.Synthetic",
            "synthetic/root.xml",
        ))
        .build()
}

#[test]
fn equal_canonical_loadout_reads_retain_one_inspection_and_every_read() -> TestResult {
    let bytes = source();
    let forge = DataForge::open(&bytes)?;
    let record = forge.records().next().ok_or("source missing")?;
    let capture = Capture::new(&ExportLedger::default(), None);
    for _ in 0..2 {
        let tree = record.tree()?;
        assert!(
            LoadoutReadObserver::observe(
                &capture,
                LoadoutReadEvent::CanonicalRead {
                    record: &record,
                    outcome: Ok(&tree),
                },
            )
            .is_continue(),
            "each actual canonical read is accepted"
        );
    }
    let ledger = capture.finish()?;
    assert_eq!(
        ledger.field_inspections.len(),
        1,
        "one retained receipt for the physical Record"
    );
    assert_eq!(
        ledger.sources.len(),
        2,
        "both observed reads remain counted"
    );
    Ok(())
}

#[test]
fn conflicting_loadout_inspections_are_refused_during_collection() -> TestResult {
    let bytes = source();
    let forge = DataForge::open(&bytes)?;
    let record = forge.records().next().ok_or("source missing")?;
    let tree = record.tree()?;
    let original = crate::export_field_walk::inspect(
        &tree,
        &record,
        Some(Kind::Loadouts),
        ExportSourceRole::Dependency,
        None,
    )?;
    let mut contradictory = original.clone();
    let inspection = contradictory
        .field_inspections
        .first_mut()
        .ok_or("inspection missing")?;
    inspection.complete = !inspection.complete;
    let mut state = State::new(&ExportLedger::default());
    state.merge_canonical(original)?;
    assert!(
        matches!(
            state.merge_canonical(contradictory),
            Err(ExportAccountingError::ConflictingFieldInspection)
        ),
        "an incompatible result cannot be coalesced"
    );
    Ok(())
}
