// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_assembly::{Entities, EntityNode, LoadoutError, LoadoutResolver};
use p4kx_extraction::{DataForge, SourceLocation, SourceUseRole};
use p4kx_testkit::{DataForgeBuilder, TestEntity, TestLoadout, TestLoadoutEntry};

use crate::Kind;
use crate::build::original_input::OriginalCatalogInput;
use crate::build::original_input::test_support::TestResult;
use crate::catalog_read::original::ExampleBodyReceipt;
use crate::catalog_read::original::health_example_retention::RetainedExampleBody;
use crate::contract_loadout::EntityKind;
use crate::export_ledger::ExportLedger;
use crate::loadout::ledger::Capture;

pub(super) fn database() -> Vec<u8> {
    p4kx_testkit::entity_schema(DataForgeBuilder::new())
        .record(
            entity(
                1,
                "ReferenceSource",
                vec![
                    TestLoadoutEntry::reference("One", 999),
                    TestLoadoutEntry::reference("Two", 999),
                ],
            )
            .record(),
        )
        .build()
}

pub(super) fn multiple_referrers() -> Vec<u8> {
    p4kx_testkit::entity_schema(DataForgeBuilder::new())
        .record(
            entity(
                1,
                "Root",
                vec![
                    TestLoadoutEntry::reference("First", 2),
                    TestLoadoutEntry::reference("Second", 3),
                ],
            )
            .record(),
        )
        .record(
            entity(
                2,
                "First",
                vec![TestLoadoutEntry::reference("Missing", 999)],
            )
            .record(),
        )
        .record(
            entity(
                3,
                "Second",
                vec![TestLoadoutEntry::reference("Missing", 999)],
            )
            .record(),
        )
        .build()
}

fn entity(guid: u128, name: &str, entries: Vec<TestLoadoutEntry>) -> TestEntity {
    TestEntity::new(guid, name, format!("{name}.xml")).loadout(TestLoadout::Manual(entries))
}

pub(super) fn body<'input>(
    forge: &'input DataForge<'input>,
    input: &'input OriginalCatalogInput<'input, 'input>,
    expected_errors: &[(u32, usize)],
) -> TestResult<ExampleBodyReceipt> {
    source_errors(forge, expected_errors)?;
    let record = forge.record(0).ok_or("the actual original root")?;
    let capture = Arc::new(Capture::for_input(
        input,
        &ExportLedger::default(),
        None,
        Kind::Loadouts,
    ));
    let mut resolver =
        LoadoutResolver::with_observed_entities(Arc::new(Entities::new(forge)), capture.clone());
    let node = resolver.resolve(&record)?;
    unresolved(&node);
    let before_factory = input.origins().uses.freeze()?;
    capture.root(&record, EntityKind::Ship, Ok(&node));
    let ledger = capture.finish()?;
    assert_eq!(
        before_factory,
        input.origins().uses.freeze()?,
        "unretained target bodies register no source uses"
    );
    let diagnostic = ledger
        .field_diagnostics
        .iter()
        .find(|row| row.warning_code.as_deref() == Some("dangling_reference"))
        .ok_or("the actual typed Reference diagnostic")?;
    assert_eq!(
        diagnostic.count, 2,
        "both actual source issues remain counted"
    );
    assert_eq!(
        diagnostic.examples.len(),
        1,
        "equal same-root examples retain one merged body"
    );
    Ok(diagnostic
        .examples
        .first()
        .ok_or("the genuine merged example")?
        .body()?
        .clone())
}

pub(super) fn retained_locations(
    body: ExampleBodyReceipt,
    input: &OriginalCatalogInput<'_, '_>,
) -> TestResult<Vec<SourceLocation>> {
    let _retained = RetainedExampleBody::read(body, input)?;
    let inventory = input.origins().uses.freeze()?;
    Ok(inventory
        .uses()
        .iter()
        .filter_map(|used| match used.location() {
            location @ SourceLocation::Subresource {
                namespace, field, ..
            } if used.role() == SourceUseRole::Payload
                && namespace == "dataforgeValue"
                && field == "$error.guid" =>
            {
                Some(location.clone())
            }
            _ => None,
        })
        .collect())
}

fn source_errors(forge: &DataForge<'_>, expected_errors: &[(u32, usize)]) -> TestResult {
    for &(index, count) in expected_errors {
        let source = forge
            .record(index)
            .ok_or("the independent original referrer")?;
        let tree = source.tree()?;
        assert!(
            tree.is_from(&source),
            "the actual original tree belongs to this referrer"
        );
        assert_eq!(
            tree.error_count(),
            count,
            "the original source retains every declared unresolved Reference"
        );
    }
    Ok(())
}

fn unresolved(node: &EntityNode) {
    let unresolved: Vec<_> = node
        .unresolved()
        .map(|(_, unresolved)| unresolved)
        .collect();
    assert_eq!(
        unresolved.len(),
        2,
        "both genuine unresolved occurrences survive the actual resolver"
    );
    assert!(
        unresolved
            .iter()
            .all(|unresolved| unresolved.reference_selection().is_some()
                && matches!(unresolved.error(), LoadoutError::DanglingReference { guid }
            if guid.guid().to_string().ends_with("03e7"))),
        "each actual SDK error remains paired with its own original getter"
    );
}
