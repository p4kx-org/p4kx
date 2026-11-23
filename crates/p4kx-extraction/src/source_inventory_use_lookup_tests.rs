// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::Value;

use crate::{
    InputScope, SourceEvidenceError, SourceInput, SourceLocation, SourceScope, SourceUse,
    SourceUseRole, SourceUses,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn selected_live_and_packed_uses_preserve_wire_order_without_expanding_inventory() -> TestResult {
    let sources = SourceUses::with_compact_coordinates();
    let scope = sources.observe(SourceInput::Unknown, InputScope::Payload)?;
    let packed = observe(&scope, 1)?;
    let live = observe(&scope, 2)?;
    observe(&scope, 3)?;
    let packed_ref = packed.reference().clone();
    let live_ref = live.reference().clone();
    let mut expected = [
        (
            packed_ref.as_str(),
            serde_json::to_value(packed.description())?,
        ),
        (live_ref.as_str(), serde_json::to_value(live.description())?),
    ];
    expected.sort_by_key(|(reference, _)| *reference);
    let expected = Value::Array(expected.into_iter().map(|(_, row)| row).collect());
    drop(packed);
    sources.compact_record_uses(|| Ok::<(), SourceEvidenceError>(()))?;
    assert_eq!(
        sources.packed_use_count()?,
        2,
        "packed includes the unselected control"
    );
    let snapshot = sources.freeze()?;
    let sealed = sources.finish_with(|| Ok::<(), SourceEvidenceError>(()))?;
    for inventory in [&snapshot, &sealed] {
        let chosen = inventory.select_uses([&live_ref, &packed_ref, &live_ref])?;
        let wire = serde_json::to_value(&chosen)?;
        assert_eq!(
            wire.get("uses"),
            Some(&expected),
            "ordered selected wire uses"
        );
        assert_eq!(
            chosen.iter_sources().len(),
            1,
            "only the selected physical source"
        );
        assert_eq!(
            chosen.iter_uses().len(),
            2,
            "repeated requests reuse the same use"
        );
        assert!(
            inventory.uses.0.expanded.get().is_none()
                && inventory.uses.0.contiguous.get().is_none(),
            "a keyed selection never expands unrelated packed rows",
        );
    }
    Ok(())
}

#[test]
fn foreign_use_with_the_same_text_is_rejected_even_after_a_valid_reference() -> TestResult {
    let sources = SourceUses::with_compact_coordinates();
    let scope = sources.observe(SourceInput::Unknown, InputScope::Payload)?;
    let actual = observe(&scope, 7)?;
    let foreign = SourceUses::with_compact_coordinates();
    let foreign_scope = foreign.observe(SourceInput::Unknown, InputScope::Payload)?;
    let forged = observe(&foreign_scope, 7)?;
    assert_eq!(
        actual.reference(),
        forged.reference(),
        "the authorities reuse the same wire text"
    );
    let actual_ref = actual.reference().clone();
    drop(actual);
    let inventory = sources.finish_with(|| Ok::<(), SourceEvidenceError>(()))?;
    assert!(
        matches!(
            inventory.select_uses([&actual_ref, forged.reference()]),
            Err(SourceEvidenceError::ForeignEvidence)
        ),
        "deduplication cannot admit a foreign use with the same wire ID",
    );
    Ok(())
}

fn observe(scope: &SourceScope, record_index: u32) -> Result<SourceUse, SourceEvidenceError> {
    scope.at(
        SourceLocation::DataForge {
            record_index,
            field: "field".into(),
        },
        SourceUseRole::Payload,
    )
}
