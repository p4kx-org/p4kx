// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::EntryOperation;
use crate::HealthText;
use crate::Kind;
use crate::catalog_read::original::EntryDiagnosis;
use crate::export_diagnostic::ExportSourceRole;
use crate::export_entry_ledger::ArchiveEntryObservation;
use crate::export_ledger::{ExportLedger, SourceDisposition};

use super::vehicle_entries;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn entry(
    operation: EntryOperation,
    index: usize,
    disposition: SourceDisposition,
) -> ArchiveEntryObservation {
    ArchiveEntryObservation {
        operation,
        kind: Kind::Ships,
        entry_index: Some(index),
        container_entry_index: None,
        path: HealthText::new(&format!("synthetic/{index}.xml")),
        role: ExportSourceRole::Dependency,
        disposition,
        diagnosis: EntryDiagnosis::default(),
        receipt: None,
    }
}

#[test]
fn paints_reuse_vehicle_evidence_without_unrelated_xml_loadout_failures() -> TestResult {
    let mut source = ExportLedger::default();
    source.incomplete_inventory(Kind::Ships);
    source.entry(entry(
        EntryOperation::XmlLoadoutBytes,
        1,
        SourceDisposition::ReadFailed,
    ));
    for operation in [
        EntryOperation::VehicleImplementationBytes,
        EntryOperation::VehicleImplementation,
    ] {
        source.entry(entry(operation, 2, SourceDisposition::Projected));
    }
    let mut paints = ExportLedger::default();
    vehicle_entries(&source, &mut paints, true);
    assert!(
        paints.incomplete_inventories.is_empty(),
        "an unrelated loadout failure does not invalidate paint input enumeration"
    );
    assert_eq!(
        paints.entries.len(),
        2,
        "only actual required vehicle read and parse observations are reused"
    );
    let evidence = crate::export_health_entries::compile(paints.entries)?;
    let counts = crate::export_health_entries::counts(Kind::Paints, &evidence.sources)?;
    assert_eq!(
        counts.dependencies, 1,
        "both operations share one actual physical dependency"
    );
    assert_eq!(
        counts.failed_dependencies, 0,
        "healthy vehicle facts remain healthy for paints"
    );
    Ok(())
}

#[test]
fn required_vehicle_read_and_parse_failures_remain_blocking_paint_dependencies() -> TestResult {
    for (operation, disposition) in [
        (
            EntryOperation::VehicleImplementationBytes,
            SourceDisposition::ReadFailed,
        ),
        (
            EntryOperation::VehiclePatch,
            SourceDisposition::ProjectionFailed,
        ),
    ] {
        let mut source = ExportLedger::default();
        source.entry(entry(operation, 3, disposition));
        let mut paints = ExportLedger::default();
        vehicle_entries(&source, &mut paints, true);
        assert!(
            paints.incomplete_inventories.contains(&Kind::Paints),
            "unreadable required vehicle inputs can hide paint configuration"
        );
        let evidence = crate::export_health_entries::compile(paints.entries)?;
        let counts = crate::export_health_entries::counts(Kind::Paints, &evidence.sources)?;
        assert_eq!(
            counts.failed_dependencies, 1,
            "the actual required file failure is retained"
        );
        let failed = evidence.failed.first().ok_or("paint dependency failure")?;
        assert_eq!(
            (failed.source.entry_index, failed.source.operation),
            (Some(3), operation),
            "the exact physical source and operation are preserved"
        );
    }
    Ok(())
}

#[test]
fn excluded_paints_do_not_invent_vehicle_file_reads() {
    let mut source = ExportLedger::default();
    source.entry(entry(
        EntryOperation::VehicleImplementationBytes,
        1,
        SourceDisposition::ReadFailed,
    ));
    let mut paints = ExportLedger::default();
    vehicle_entries(&source, &mut paints, false);
    assert!(
        paints.entries.is_empty() && paints.incomplete_inventories.is_empty(),
        "no paint candidates means no paint-specific vehicle operation was required"
    );
}
