// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::ControlFlow;
use std::sync::Arc;

use p4kx_assembly::{LoadoutReadEvent, LoadoutReadObserver};
use p4kx_extraction::{DataForge, DataForgeError, Record, SourceEvidenceError, TreeBuilder, Value};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

use super::BuilderCapture;
use crate::build::original_input::test_support::{self, TestResult};
use crate::export_diagnostic::ExportSourceRole;
use crate::{CatalogError, Kind};

fn database() -> Vec<u8> {
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("Source").field("value", TestType::UInt32))
        .record(
            TestRecord::new(0x6331, "Source", "Source.One", "source.xml")
                .value("value", TestValue::UInt32(17)),
        )
        .build()
}

#[test]
fn borrowed_builder_capture_observes_original_reads_and_retains_first_foreign_cause() -> TestResult
{
    let bytes = database();
    let foreign = DataForge::open(&bytes)?;
    let other = foreign.record(0).ok_or("actual foreign Record")?;
    let other_tree = other.tree()?;
    assert_eq!(
        other_tree.field("value"),
        Some(&Value::UInt32(17)),
        "actual foreign original payload"
    );
    test_support::with_input(&bytes, |forge, input| {
        let record = forge.record(0).ok_or("actual admitted Record")?;
        same_legacy_headers(&record, &other);
        let capture = Arc::new(BuilderCapture::for_input_kind(input, Kind::Items, None));
        let mut trees = TreeBuilder::with_read_observer(capture.clone());
        let tree = trees.build(&record)?;
        assert!(
            tree.is_from(&record),
            "the observed tree retains its actual Forge owner"
        );
        let ledger = capture.finish()?;
        assert_eq!(
            ledger.sources.len(),
            1,
            "the real borrowed observer records the actual read"
        );
        assert_eq!(
            ledger.field_inspections.len(),
            1,
            "canonical numeric/field inspection runs once"
        );
        assert!(
            !other.is_from(forge),
            "the foreign source is independently opened"
        );
        assert!(
            matches!(trees.build(&other), Err(DataForgeError::ObservationStopped)),
            "foreign admission stops the actual read result"
        );
        assert!(
            matches!(
                trees.build(&record),
                Err(DataForgeError::ObservationStopped)
            ),
            "the same first fatal cause stops further observer contributions"
        );
        assert!(
            matches!(
                capture.finish(),
                Err(CatalogError::SourceEvidence(
                    SourceEvidenceError::ForeignEvidence
                ))
            ),
            "the original foreign-admission cause is preserved"
        );
        Ok(())
    })
}

#[test]
fn borrowed_loadout_capture_rejects_wrong_original_tree_before_cached_proof() -> TestResult {
    let bytes = database();
    let foreign = DataForge::open(&bytes)?;
    let other = foreign.record(0).ok_or("actual foreign original")?;
    let other_tree = other.tree()?;
    test_support::with_input(&bytes, |forge, input| {
        let record = forge.record(0).ok_or("actual selected original")?;
        let tree = record.tree()?;
        assert_eq!(
            tree, other_tree,
            "legacy structural equality excludes original Forge authority"
        );
        assert!(
            !other_tree.is_from(&record),
            "the wrong tree is genuinely foreign"
        );
        let source = crate::export_field_walk::inspect(
            &tree,
            &record,
            Some(Kind::Ships),
            ExportSourceRole::Selected,
            None,
        )?;
        let capture = crate::loadout::ledger::Capture::for_input(input, &source, None, Kind::Ships);
        assert!(
            LoadoutReadObserver::observe(
                &capture,
                LoadoutReadEvent::ProvidedTree {
                    record: &record,
                    tree: &other_tree
                }
            )
            .is_break(),
            "a foreign tree cannot reuse the original cached source proof"
        );
        assert_eq!(
            LoadoutReadObserver::observe(&capture, LoadoutReadEvent::Reused { record: &other }),
            ControlFlow::Break(()),
            "later cache events remain stopped"
        );
        assert!(
            matches!(
                capture.finish(),
                Err(CatalogError::SourceEvidence(
                    SourceEvidenceError::ForeignEvidence
                ))
            ),
            "the first wrong-tree cause survives the later stop"
        );
        Ok(())
    })
}

fn same_legacy_headers(record: &Record<'_>, other: &Record<'_>) {
    assert_eq!(
        record.guid(),
        other.guid(),
        "equal original GUIDs are not input authority"
    );
    assert_eq!(
        record.index(),
        other.index(),
        "equal original indices are not input authority"
    );
}
