// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_assembly::{Entities, LoadoutError, LoadoutResolver};
use p4kx_extraction::{PhysicalSourceRef, SourceLocation, SourceUseDescription, SourceUseRole};

use super::probe::Probe;
use crate::build::original_input::test_support::TestResult;
use crate::build::original_input::{OriginalCatalogInput, ship_pool_test_input};
use crate::catalog_read::original::ExampleBodyReceipt;
use crate::catalog_read::original::health_example_retention::RetainedExampleBody;
use crate::contract_loadout::EntityKind;
use crate::export_ledger::ExportLedger;

#[path = "health_loadout_class_scope_fixture.rs"]
mod fixture;

#[test]
fn an_xml_class_cycle_keeps_the_actual_third_caller_and_excludes_the_previous_middle_caller()
-> TestResult {
    with_original(|input, body, _, _, _| {
        let contributions = target_uses(input, body)?;
        assert!(
            contributions.iter().any(|used| record_selector(used, 2)),
            "the actual current third caller selects this class"
        );
        assert!(
            !contributions.iter().any(|used| record_selector(used, 1)),
            "the earlier independent root cache caller cannot select this class"
        );
        Ok(())
    })
}

#[test]
fn an_xml_class_cycle_excludes_the_losing_shared_child_branch() -> TestResult {
    with_original(|input, body, left, right, sources| {
        let contributions = target_uses(input, body)?;
        assert!(
            contributions
                .iter()
                .any(|used| include_selector(used, &sources.root, right)),
            "the original Root to Right selector supplies the actual winning branch"
        );
        assert!(
            contributions
                .iter()
                .any(|used| include_selector(used, &sources.right, left)),
            "the winning Right to Shared selector has the same node ordinal as Root to Left, in another original Entry"
        );
        assert!(
            !contributions
                .iter()
                .any(|used| include_selector(used, &sources.root, left)),
            "the shadowed Root to Left branch did not supply the selected class"
        );
        Ok(())
    })
}

fn with_original(
    check: impl FnOnce(
        &OriginalCatalogInput<'_, '_>,
        ExampleBodyReceipt,
        u64,
        u64,
        fixture::IncludeSources,
    ) -> TestResult,
) -> TestResult {
    ship_pool_test_input::with_input(
        &fixture::database(),
        fixture::entries(),
        |archive, files, forge, input| {
            let (guid, left, right) = fixture::original(archive, forge)?;
            let probe = Arc::new(Probe::new(input));
            let mut resolver = LoadoutResolver::with_observed_entities(
                Arc::new(Entities::new(forge)),
                probe.clone(),
            )
            .with_files(files);
            let mut third = None;
            for index in 0..3 {
                let record = forge.record(index).ok_or("the actual source Record")?;
                let node = resolver.resolve(&record)?;
                let mut unresolved = node.unresolved();
                let (_, first) = unresolved
                    .next()
                    .ok_or("the retained Right branch class cycle")?;
                assert!(
                    matches!(first.error(), LoadoutError::Cycle { guid: target } if target.guid() == guid
                    && target.class_selection().is_some()),
                    "the actual winner points to the original Self definition"
                );
                assert!(
                    unresolved.next().is_none(),
                    "the shadowed left branch did not yield a second result"
                );
                probe.capture.root(&record, EntityKind::Ship, Ok(&node));
                let ledger = probe.capture.finish()?;
                if let Some(body) = third_cycle(&ledger, index)? {
                    third = Some(body);
                }
            }
            check_reuse(&probe)?;
            let sources = fixture::include_sources(archive, &input.origins().uses)?;
            check(
                input,
                third.ok_or("the actual third caller body")?,
                left,
                right,
                sources,
            )
        },
    )
}

fn third_cycle(ledger: &ExportLedger, index: u32) -> TestResult<Option<ExampleBodyReceipt>> {
    let diagnostic = ledger
        .field_diagnostics
        .iter()
        .find(|row| row.warning_code.as_deref() == Some("cycle"))
        .ok_or("the actual class cycle diagnostic")?;
    assert_eq!(diagnostic.count, 1, "one actual cycle per caller");
    assert_eq!(
        diagnostic.examples.len(),
        1,
        "one genuine class example per caller"
    );
    let example = diagnostic
        .examples
        .first()
        .ok_or("the actual class example")?;
    if index == 2 {
        return Ok(Some(example.body()?.clone()));
    }
    Ok(None)
}

fn check_reuse(probe: &Probe<'_>) -> TestResult {
    probe.file_counts(4, 2, 4);
    let selections = probe
        .selections
        .lock()
        .map_err(|_| "actual class selection observations")?;
    assert_eq!(
        selections.len(),
        3,
        "the actual winner is selected for each resolution"
    );
    let original = selections.first().ok_or("the actual first getter")?;
    assert!(
        selections.iter().all(|candidate| candidate == original),
        "the actual cached XML getter is shared"
    );
    drop(selections);
    Ok(())
}

fn target_uses(
    input: &OriginalCatalogInput<'_, '_>,
    body: ExampleBodyReceipt,
) -> TestResult<Vec<SourceUseDescription>> {
    let retained = RetainedExampleBody::read(body, input)?;
    let uses = &input.origins().uses;
    let texts = input
        .origins()
        .health_texts()
        .seal(uses, None, &retained.texts())?;
    let cells = retained.finish(&texts, uses)?;
    let inventory = uses.freeze()?;
    let contributions: Vec<_> = inventory
        .uses()
        .iter()
        .filter(|used| {
            cells
                .target_guid
                .origin()
                .source_uses_refs()
                .contains(used.id())
        })
        .cloned()
        .collect();
    assert!(contributions.iter().any(|used| used.role() == SourceUseRole::Payload
        && matches!(used.location(), SourceLocation::Subresource { namespace, indices, field }
            if namespace == "dataforgeRecordHeader" && indices == &[0] && field == "guid")),
        "the actual first physical Record supplies this target GUID");
    assert!(contributions.iter().any(|used| used.role() == SourceUseRole::Selector
        && matches!(used.location(), SourceLocation::Subresource { namespace, indices, field }
            if namespace == "xmlAttribute" && indices == &[3, 1] && field == "@itemName")),
        "the actual cached physical Item getter is retained");
    Ok(contributions)
}

fn record_selector(used: &SourceUseDescription, record: u64) -> bool {
    used.role() == SourceUseRole::Selector
        && matches!(used.location(),
        SourceLocation::Subresource { namespace, indices, field }
            if namespace == "dataforgeValue" && indices.first() == Some(&record) && field == "$value")
}

fn include_selector(used: &SourceUseDescription, source: &PhysicalSourceRef, node: u64) -> bool {
    used.role() == SourceUseRole::Selector
        && used.source() == source
        && matches!(used.location(),
        SourceLocation::Subresource { namespace, indices, field }
            if namespace == "xml" && indices == &[node] && field == "@loadout")
}
