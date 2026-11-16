// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;

use p4kx_extraction::{Guid, Record, TreeBuilder};
use p4kx_testkit::{TestRecord, TestStruct, TestType, TestValue};

use crate::CatalogError;
use crate::build::original_input::{OriginalCatalogInput, test_support};
use crate::crafting_sources;
use crate::export_dependency_read;
use crate::export_ledger::SourceDisposition;
use crate::record_observation::RecordObservation;

use super::super::{ExportLedger, Kind, RewardSearch};
use super::{CONTRACT, TestResult, reward, rewards_fixture};

fn mixed_rewards() -> Vec<u8> {
    rewards_fixture()
        .structure(
            TestStruct::new("Malformed")
                .field("value", TestType::Int32)
                .declared_size(6),
        )
        .structure(
            TestStruct::new("Diagnostics")
                .field("bad", TestType::Bool)
                .field("link", TestType::Reference),
        )
        .record(
            TestRecord::new(
                CONTRACT + 1,
                "Contract",
                "Contract.Other",
                "synthetic/other.xml",
            )
            .value("rewards", TestValue::Array(vec![reward(-0.0)])),
        )
        .record(
            TestRecord::new(
                CONTRACT + 2,
                "Diagnostics",
                "Diagnostics.Synthetic",
                "synthetic/diagnostics.xml",
            )
            .value("bad", TestValue::Raw(vec![2]))
            .value("link", TestValue::Reference(CONTRACT + 4)),
        )
        .record(TestRecord::new(
            CONTRACT + 3,
            "Malformed",
            "Malformed.Synthetic",
            "synthetic/malformed.xml",
        ))
        .build()
}

#[test]
fn ordered_chunks_match_sequential_placements_and_receipts() -> TestResult {
    let bytes = mixed_rewards();
    test_support::with_same_forge_reports(&bytes, |forge, sequential, parallel| {
        let records: Vec<_> = forge.records().collect();
        let wanted = HashSet::from([records.first().ok_or("pool fixture")?.guid()]);
        let expected = sequential_sources(&records, &wanted, sequential)?;
        assert_eq!(
            expected.sources.len(),
            3,
            "two original placements from one owner and one from another",
        );
        assert!(
            expected
                .ledger
                .sources
                .iter()
                .any(|source| source.disposition == SourceDisposition::ReadFailed),
            "the fixture includes an actual record read failure",
        );
        assert!(
            expected.ledger.field_inspections.iter().any(|field| {
                !field.findings.diagnostics().is_empty()
                    && !field.findings.unresolved_references().is_empty()
            }),
            "the fixture includes actual diagnostics and reference receipts",
        );
        for size in [1, 2, records.len()] {
            let measured =
                super::super::scan_chunks(&records, size, &wanted, Some(parallel), None, None)?;
            check_receipts(&measured, &expected, parallel)?;
            assert_eq!(
                parallel.origins().uses.freeze()?,
                sequential.origins().uses.freeze()?,
                "the same archive and coordinates retain identical provenance",
            );
        }
        Ok(())
    })
}

fn check_receipts(
    measured: &RewardSearch<'_>,
    expected: &RewardSearch<'_>,
    input: &OriginalCatalogInput<'_, '_>,
) -> TestResult {
    assert_eq!(measured.sources, expected.sources, "exact placement order");
    assert!(
        measured
            .sources
            .iter()
            .zip(&expected.sources)
            .all(|(left, right)| {
                left.chance.map(f32::to_bits) == right.chance.map(f32::to_bits)
            }),
        "retained chance values preserve their exact float bits",
    );
    assert_eq!(measured.unreadable, expected.unreadable, "warning count");
    assert_eq!(
        measured.ledger.sources, expected.ledger.sources,
        "physical source outcomes"
    );
    assert_eq!(
        measured.ledger.field_inspections, expected.ledger.field_inspections,
        "ordered original field inspections, diagnostics and references",
    );
    for source in &measured.ledger.sources {
        source.subject().check_input(input)?;
    }
    for field in &measured.ledger.field_inspections {
        field.subject.check_input(input)?;
    }
    Ok(())
}

fn sequential_sources<'f>(
    records: &[Record<'f>],
    wanted: &HashSet<Guid>,
    input: &OriginalCatalogInput<'_, '_>,
) -> Result<RewardSearch<'f>, CatalogError> {
    let mut trees = TreeBuilder::new();
    let mut ledger = ExportLedger::default();
    let mut sources = Vec::new();
    let mut types = HashSet::new();
    let mut failures = Vec::new();
    for record in records {
        let admission = RecordObservation::capture(input, record)?;
        let dependency = export_dependency_read::read(
            Some(input),
            &mut trees,
            &admission,
            Kind::BlueprintPools,
            None,
        )?;
        ledger.merge(dependency.ledger);
        match dependency.tree {
            Ok(tree) => {
                let mut placements = Vec::new();
                crafting_sources::collect(&tree, wanted, &mut placements);
                if !placements.is_empty() {
                    types.insert(tree.type_name());
                    sources.extend(placements);
                }
            }
            Err(_) => failures.push(record.type_name().ok()),
        }
    }
    let unreadable = if sources.is_empty() {
        failures.len()
    } else {
        failures
            .into_iter()
            .flatten()
            .filter(|name| !name.is_empty() && types.contains(name))
            .count()
    };
    Ok(RewardSearch {
        sources,
        unreadable,
        ledger,
    })
}
