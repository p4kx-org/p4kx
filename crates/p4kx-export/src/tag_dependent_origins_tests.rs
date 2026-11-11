// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;
use std::sync::Arc;

use p4kx_extraction::{SourceUse, SourceUseRef, SourceUseRole, SourceUses, SourceValue};
use serde_json::Value;

use super::{Cell, ContextualSourcedValue, SelectedPaths, TagInputs};
use crate::build::original_input::test_support::{self, TestResult};

#[path = "tag_dependent_origins_fixture.rs"]
mod fixture;
#[path = "tag_dependent_origins_guards.rs"]
mod guards;

#[test]
fn repeated_physical_tag_operands_stay_compact_until_the_value_is_written() -> TestResult {
    test_support::with_same_forge_reports(&fixture::bytes(), |forge, input, _| {
        let inputs = fixture::selected(forge, input)?;
        let selected = check_compact(&inputs)?;
        let sources = &input.origins().uses;
        let before = sources.freeze()?;
        let cells = (0..fixture::CALLERS)
            .map(|_| {
                inputs
                    .clone()
                    .bind(SourceValue::Present(fixture::CALLERS), sources)
            })
            .collect::<Result<Vec<_>, _>>()?;
        for cell in &cells {
            let Cell::Deferred {
                inputs,
                sources: retained,
                ..
            } = &cell.value
            else {
                return Err("the selected lineage must remain deferred".into());
            };
            let actual = check_compact(inputs)?;
            assert!(
                Arc::ptr_eq(&actual.receipts, &selected.receipts),
                "all retained values share the original selected arena"
            );
            assert!(
                sources.is_same_inventory(retained),
                "the deferred cell keeps its report"
            );
        }
        assert_eq!(
            before,
            sources.freeze()?,
            "binding adds no invented observations"
        );
        check_writers(&cells, &inputs, sources)?;
        assert_eq!(
            before,
            sources.freeze()?,
            "serialization retains only already observed uses"
        );
        Ok(())
    })
}

fn check_compact(inputs: &TagInputs) -> TestResult<&SelectedPaths> {
    assert_eq!(
        inputs.local.len(),
        fixture::CALLERS,
        "one local observation per original operand"
    );
    assert_eq!(
        references(&inputs.local).len(),
        fixture::CALLERS,
        "repeated values retain distinct slots"
    );
    assert!(
        inputs
            .local
            .iter()
            .all(|used| used.role() == SourceUseRole::Selector),
        "the reference operands select the derived value"
    );
    let selected = inputs
        .selected
        .as_ref()
        .ok_or("actual selected path positions")?;
    assert_eq!(
        selected.first,
        fixture::STEM,
        "the first original operand selects the left leaf"
    );
    assert_eq!(
        selected.other,
        BTreeSet::from([fixture::STEM + 1]),
        "repeated operands store only the other distinct selected position"
    );
    assert_eq!(
        selected.receipts.len(),
        fixture::TAGS,
        "one arena stores the whole physical tree"
    );
    assert!(
        inputs.complete,
        "the controlled original operands are complete"
    );
    Ok(selected)
}

fn check_writers(
    cells: &[ContextualSourcedValue<usize>],
    inputs: &TagInputs,
    sources: &SourceUses,
) -> TestResult {
    let selected = inputs.selected.as_ref().ok_or("actual selected arena")?;
    let mut lineage = Vec::new();
    selected.receipts.append_path_union(
        std::iter::once(selected.first).chain(selected.other.iter().copied()),
        &mut lineage,
    )?;
    assert_eq!(
        lineage.len(),
        fixture::TAGS * 5 + 3,
        "each unique physical ancestor contributes once despite overlapping paths"
    );
    assert_eq!(
        lineage.len(),
        references(&lineage).len(),
        "the temporary writer union has no repeated lineage observations"
    );
    let mut expanded = inputs.local.clone();
    expanded.extend(lineage);
    let expected = serde_json::to_value(sources.value(
        SourceValue::Present(fixture::CALLERS),
        &expanded,
        true,
    ))?;
    for cell in cells {
        let first = serde_json::to_value(cell)?;
        assert_eq!(
            first, expected,
            "the compact writer preserves the full sourced cell"
        );
        assert_eq!(
            serde_json::to_value(cell)?,
            first,
            "serialization can be repeated"
        );
        assert_eq!(first.get("sourceSetComplete"), Some(&Value::Bool(true)));
        assert_eq!(
            first.get("containsEncryptedSource"),
            Some(&Value::Bool(false))
        );
    }
    Ok(())
}

fn references(uses: &[SourceUse]) -> BTreeSet<&SourceUseRef> {
    uses.iter().map(SourceUse::reference).collect()
}
