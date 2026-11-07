// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::DataForge;
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

use crate::export_diagnostic::ExportSourceRole;
use crate::export_ledger::SourceDisposition;

use super::{BlueprintPool, ExportLedger, Guid, Kind, RewardSource, Scanned};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn source(path: &str) -> RewardSource<'_> {
    RewardSource {
        record: Guid::default(),
        record_path: path,
        contract_id: None,
        contract_name: None,
        pool: Guid::default(),
        chance: Some(1.0),
    }
}

fn read<'f>(type_name: &'f str, sources: Vec<RewardSource<'f>>) -> Scanned<'f> {
    Scanned::Read {
        type_name,
        sources,
        ledger: ExportLedger::default(),
    }
}

fn failed(type_name: &str) -> Scanned<'_> {
    Scanned::Failed {
        type_name: Some(type_name),
        ledger: ExportLedger::default(),
    }
}

#[test]
fn an_unreadable_record_counts_only_when_its_type_holds_a_source_elsewhere() {
    let search = super::gather(vec![
        read("ContractGenerator", vec![source("a"), source("b")]),
        failed("ContractGenerator"),
        failed("ContractGenerator"),
        failed("ShipLoadout"),
        read("Nothing", Vec::new()),
    ]);
    assert_eq!(
        search.unreadable, 2,
        "the unrelated type is not in the compatibility warning"
    );
    assert_eq!(
        search.sources.len(),
        2,
        "both actual source placements remain"
    );
}

#[test]
fn without_any_source_every_unreadable_record_counts() {
    let search = super::gather(vec![
        failed("ContractGenerator"),
        failed("ShipLoadout"),
        read("Nothing", Vec::new()),
    ]);
    assert_eq!(
        search.unreadable, 2,
        "any failed Record could hold an undiscovered source"
    );
    assert!(
        search.sources.is_empty(),
        "no successful source was fabricated"
    );
}

#[test]
fn records_without_a_type_name_do_not_match_each_other() {
    let search = super::gather(vec![read("", vec![source("a")]), failed("")]);
    assert_eq!(
        search.unreadable, 0,
        "an unavailable type is not positive type evidence"
    );
}

#[test]
fn sources_keep_the_order_of_the_records() {
    let search = super::gather(vec![
        read("A", vec![source("first")]),
        read("Nothing", Vec::new()),
        read("B", vec![source("second"), source("third")]),
    ]);
    let paths: Vec<_> = search
        .sources
        .iter()
        .map(|source| source.record_path)
        .collect();
    assert_eq!(
        paths,
        ["first", "second", "third"],
        "parallel scanning preserves input Record order"
    );
}

const POOL: u128 = 0x6600_0000_0000_4000_8000_0000_0000_0010;
const CONTRACT: u128 = POOL + 1;

fn rewards() -> Vec<u8> {
    let reward = || {
        TestValue::Struct(
            TestInstance::new("Reward")
                .value("pool", TestValue::Reference(POOL))
                .value("chance", TestValue::Single(0.5)),
        )
    };
    DataForgeBuilder::new()
        .structure(TestStruct::new("Pool"))
        .structure(
            TestStruct::new("Reward")
                .field("pool", TestType::Reference)
                .field("chance", TestType::Single),
        )
        .structure(
            TestStruct::new("Contract").array("rewards", TestType::Class("Reward".to_owned())),
        )
        .record(TestRecord::new(
            POOL,
            "Pool",
            "Pool.Synthetic",
            "synthetic/pool.xml",
        ))
        .record(
            TestRecord::new(
                CONTRACT,
                "Contract",
                "Contract.Synthetic",
                "synthetic/contract.xml",
            )
            .value("rewards", TestValue::Array(vec![reward(), reward()])),
        )
        .build()
}

#[test]
fn two_reward_placements_are_one_actual_dependency_record() -> TestResult {
    let bytes = rewards();
    let forge = DataForge::open(&bytes)?;
    let records: Vec<_> = forge.records().collect();
    let pool = records.first().ok_or("pool fixture missing")?;
    let pools = [BlueprintPool {
        guid: pool.guid(),
        class_name: "Pool.Synthetic",
        record_path: "synthetic/pool.xml",
        entries: Vec::new(),
    }];
    let search = super::reward_sources(&records, &pools, None, None)?;
    assert_eq!(
        search.sources.len(),
        2,
        "both reward placements remain in the public result"
    );
    assert_eq!(
        search.ledger.sources.len(),
        2,
        "each checked Record has one actual source outcome"
    );
    let contract = records.get(1).ok_or("contract fixture missing")?;
    assert_eq!(
        search
            .ledger
            .sources
            .iter()
            .filter(|source| source.guid == contract.guid())
            .count(),
        1,
        "emitted reward rows do not multiply source-Record counters"
    );
    assert!(
        search
            .ledger
            .sources
            .iter()
            .all(|source| source.kind == Some(Kind::BlueprintPools)
                && source.role == ExportSourceRole::Dependency
                && source.disposition == SourceDisposition::Projected),
        "every successful canonical scan has a typed dependency outcome"
    );
    assert_eq!(
        search.ledger.field_inspections.len(),
        2,
        "both canonical inputs were inspected"
    );
    Ok(())
}

#[test]
fn an_actual_failed_reward_dependency_retains_its_guid_and_path() -> TestResult {
    let bytes = crate::crafting::ledger::fixtures::broken(
        "Contract",
        "Contract.Synthetic",
        "synthetic/contract.xml",
    );
    let forge = DataForge::open(&bytes)?;
    let records: Vec<_> = forge.records().collect();
    let search = super::reward_sources(&records, &[], None, None)?;
    let source = search
        .ledger
        .sources
        .first()
        .ok_or("failed dependency missing")?;
    assert_eq!(
        source.guid,
        records.first().ok_or("Record missing")?.guid(),
        "the failed source GUID is retained"
    );
    assert_eq!(
        source.record_path.as_deref(),
        Some("synthetic/contract"),
        "the failure path is retained"
    );
    assert_eq!(
        source.disposition,
        SourceDisposition::ReadFailed,
        "the canonical read actually failed"
    );
    assert_eq!(
        source.role,
        ExportSourceRole::Dependency,
        "the failure belongs to pool dependency discovery"
    );
    assert!(
        search.ledger.field_inspections.is_empty(),
        "a failed canonical read has no invented inspection"
    );
    assert_eq!(
        search.unreadable, 1,
        "the compatibility warning still counts the failed search"
    );
    Ok(())
}

#[test]
fn unreadable_field_leaves_are_retained_even_without_reward_rows() -> TestResult {
    let bytes = crate::crafting::ledger::fixtures::fields();
    let forge = DataForge::open(&bytes)?;
    let records: Vec<_> = forge.records().collect();
    let search = super::reward_sources(&records, &[], None, None)?;
    assert!(
        search.sources.is_empty(),
        "a source-free canonical scan has no invented reward rows"
    );
    assert_eq!(
        search
            .ledger
            .sources
            .first()
            .ok_or("source outcome missing")?
            .disposition,
        SourceDisposition::Projected,
        "the actual successful zero-row scan is retained"
    );
    assert!(
        search
            .ledger
            .field_inspections
            .iter()
            .flat_map(|inspection| &inspection.diagnostics)
            .any(|diagnostic| diagnostic.code
                == crate::export_diagnostic::ExportDiagnosticCode::UnreadableSource),
        "field decoding errors survive optional projection helpers"
    );
    Ok(())
}
