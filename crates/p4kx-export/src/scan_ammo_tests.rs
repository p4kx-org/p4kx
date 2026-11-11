// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::atomic::Ordering;

use p4kx_extraction::{DataForgeError, Filter};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

use super::AmmoReads;
use crate::Kind;
use crate::build::original_input::test_support::{self, TestResult};
use crate::export_diagnostic::ExportSourceRole;
use crate::export_ledger::{ExportLedger, SourceDisposition};
use crate::scan_health::ScanError;

#[test]
fn ammo_memo_materializes_one_genuine_physical_tree_across_roles() -> TestResult {
    test_support::with_input(&bytes(), |forge, input| {
        let first = forge.record(0).ok_or("first physical definition")?;
        let second = forge.record(1).ok_or("second physical definition")?;
        let reads = AmmoReads::new(input, None, None);
        let mut dependency = ExportLedger::default();
        let compatibility = reads
            .dependency(&first, &mut dependency)?
            .ok_or("actual legacy ammunition")?;
        assert_eq!(compatibility.speed, Some(11.0), "original finite speed");
        let mut repeated = ExportLedger::default();
        reads
            .dependency(&first, &mut repeated)?
            .ok_or("same dependency")?;
        let mut selected = ExportLedger::default();
        let row = reads.selected(&first, &mut selected)?;
        assert_eq!(row.physical_index, 0, "selected same original Record");
        assert_eq!(
            reads.materialized_reads.load(Ordering::Relaxed),
            1,
            "one physical tree build"
        );
        assert!(
            repeated.field_inspections.is_empty(),
            "shared dependency has one inspection"
        );
        assert!(
            repeated.sources.is_empty(),
            "shared dependency counted once"
        );
        assert_roles(&selected, &dependency)?;
        let other = reads.selected(&second, &mut selected)?;
        assert_eq!(
            row.physical_guid, other.physical_guid,
            "fixture duplicate nominal GUID"
        );
        assert_eq!(
            other.physical_index, 1,
            "another physical Record keeps its own row"
        );
        assert_eq!(
            reads.materialized_reads.load(Ordering::Relaxed),
            2,
            "one build per physical index"
        );
        Ok(())
    })
}

#[test]
fn ammo_memo_preserves_filtered_dependency_and_caches_actual_read_failure() -> TestResult {
    test_support::with_input(&bytes(), |forge, input| {
        let filtered = forge.record(0).ok_or("filtered original Ammo")?;
        let malformed = forge.record(2).ok_or("malformed original Ammo")?;
        let filter = Filter::parse("*/selected_*")?;
        let reads = AmmoReads::new(input, Some(&filter), None);
        let mut ledger = ExportLedger::default();
        let compatibility = reads
            .dependency(&filtered, &mut ledger)?
            .ok_or("filtered dependency remains readable")?;
        assert_eq!(compatibility.speed, Some(11.0), "original dependency value");
        assert!(
            matches!(
                reads.selected(&filtered, &mut ledger),
                Err(ScanError::Fatal(_))
            ),
            "a dependency cannot manufacture a selected Ammo row"
        );
        let failed = reads.dependency(&malformed, &mut ledger)?;
        assert!(failed.is_none(), "unreadable dependency leaves legacy null");
        let Err(ScanError::Source(error)) = reads.selected(&malformed, &mut ledger) else {
            return Err("actual canonical source read failure".into());
        };
        assert!(
            matches!(
                error,
                DataForgeError::StructLayout {
                    declared: 3,
                    fields: 2,
                    ..
                }
            ),
            "the actual canonical reader rejects the original malformed layout"
        );
        assert_eq!(
            reads.materialized_reads.load(Ordering::Relaxed),
            2,
            "one attempted read of each demanded physical Record"
        );
        assert!(
            ledger.sources.iter().any(|source| {
                source.record_index == 2
                    && source.role == ExportSourceRole::Dependency
                    && source.disposition == SourceDisposition::ReadFailed
            }),
            "dependency failure reaches the original shared ledger"
        );
        Ok(())
    })
}

fn assert_roles(selected: &ExportLedger, dependency: &ExportLedger) -> TestResult {
    let [selected] = selected.field_inspections.as_slice() else {
        return Err("one selected canonical field inspection".into());
    };
    let [dependency] = dependency.field_inspections.as_slice() else {
        return Err("one dependency canonical field inspection".into());
    };
    assert_eq!(selected.kind, Some(Kind::AmmoDefinitions), "selected kind");
    assert_eq!(selected.role, ExportSourceRole::Selected, "selected role");
    assert_eq!(dependency.kind, Some(Kind::Items), "consuming kind");
    assert_eq!(
        dependency.role,
        ExportSourceRole::Dependency,
        "dependency role"
    );
    let mut neutral = selected.clone();
    neutral.contextualize(Some(Kind::Items), ExportSourceRole::Dependency);
    assert_eq!(
        &neutral, dependency,
        "both roles use the same physical inspection"
    );
    Ok(())
}

fn bytes() -> Vec<u8> {
    DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new("AmmoParams")
                .field("speed", TestType::Single)
                .field(
                    "trap",
                    TestType::StrongPointer("FixtureInvalidLayout".to_owned()),
                ),
        )
        .structure(
            TestStruct::new("FixtureInvalidLayout")
                .field("first", TestType::Int8)
                .field("second", TestType::Int8)
                .declared_size(3),
        )
        .record(ammo(0x8110, "first", 11.0))
        .record(ammo(0x8110, "second", 12.0))
        .record(ammo(0x8111, "selected_malformed", 13.0).value(
            "trap",
            TestValue::Struct(TestInstance::new("FixtureInvalidLayout")),
        ))
        .build()
}

fn ammo(guid: u128, name: &str, speed: f32) -> TestRecord {
    TestRecord::new(
        guid,
        "AmmoParams",
        format!("AmmoParams.{name}"),
        format!("fixture/{name}.xml"),
    )
    .value("speed", TestValue::Single(speed))
}
