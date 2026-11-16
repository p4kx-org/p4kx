// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::{
    InputScope, SourceEvidenceError, SourceInput, SourceLocation, SourceUse, SourceUseRole,
    SourceUses, SourceValue,
};

use super::SourceInventoryFreezeError;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn use_at(sources: &SourceUses, record_index: u32) -> Result<SourceUse, SourceEvidenceError> {
    sources
        .observe(SourceInput::Unknown, InputScope::Payload)?
        .at(
            SourceLocation::DataForge {
                record_index,
                field: "source".into(),
            },
            SourceUseRole::Payload,
        )
}

#[test]
fn finishing_moves_sorted_rows_and_preserves_captured_value_binding() -> TestResult {
    let sources = SourceUses::with_compact_coordinates();
    let same = sources.clone();
    let first = use_at(&sources, 9)?;
    let second = use_at(&sources, 1)?;
    let foreign = use_at(&SourceUses::with_compact_coordinates(), 9)?;
    let own = [second.clone(), first];
    let mixed = [second.clone(), foreign];
    let previous = sources.freeze()?;
    let expected = sources.value(SourceValue::Present(42), &own, true);
    let expected_mixed = sources.value(SourceValue::Present(42), &mixed, true);

    let inventory = sources.finish_with(|| Ok::<(), SourceEvidenceError>(()))?;
    assert_eq!(
        inventory, previous,
        "same inventory as the repeatable snapshot"
    );
    assert!(
        inventory
            .iter_uses()
            .any(|row| std::ptr::eq(row, second.description())),
        "capture keeps the original registered Arc row"
    );
    assert_eq!(same.value(SourceValue::Present(42), &own, true), expected);
    assert_eq!(
        same.value(SourceValue::Present(42), &mixed, true),
        expected_mixed,
        "foreign evidence is still rejected during deferred value binding"
    );
    assert!(
        !expected_mixed.origin().source_set_complete(),
        "a foreign contributor does not become complete evidence"
    );
    assert!(
        matches!(same.freeze(), Err(SourceEvidenceError::SealedInventory)),
        "the terminal finish seals every cloned registry handle"
    );
    Ok(())
}

#[test]
fn interruption_before_seal_leaves_registration_available() -> TestResult {
    let sources = SourceUses::new();
    use_at(&sources, 1)?;
    let cancelled = sources.finish_with(|| Err(SourceInventoryFreezeError::Interrupted(())));
    assert!(
        matches!(cancelled, Err(SourceInventoryFreezeError::Interrupted(()))),
        "the first checkpoint prevents the terminal operation"
    );
    use_at(&sources, 2)?;
    assert_eq!(
        sources.freeze()?.use_count(),
        2,
        "registration remains open"
    );
    Ok(())
}

#[test]
fn interruption_while_packing_leaves_the_registry_sealed() -> TestResult {
    let sources = SourceUses::with_compact_use_refs();
    let scope = sources.observe(SourceInput::Unknown, InputScope::Payload)?;
    use_at(&sources, 1)?;
    let mut check = 0;
    let cancelled = sources.finish_with(|| {
        check += 1;
        if check == 1 {
            Ok(())
        } else {
            Err(SourceInventoryFreezeError::Interrupted(()))
        }
    });
    assert!(
        matches!(cancelled, Err(SourceInventoryFreezeError::Interrupted(()))),
        "interruption cannot return a partially packed inventory"
    );
    assert_eq!(check, 2, "the operation entered terminal row packing");
    assert!(
        matches!(
            sources.observe(SourceInput::Unknown, InputScope::Payload),
            Err(SourceEvidenceError::SealedInventory)
        ),
        "a new source cannot enter the sealed inventory"
    );
    assert!(
        matches!(
            scope.at(
                SourceLocation::Metadata {
                    field: "path".into()
                },
                SourceUseRole::Payload,
            ),
            Err(SourceEvidenceError::SealedInventory)
        ),
        "an existing scope cannot register a new use"
    );
    assert!(
        matches!(scope.ancestor(0), Err(SourceEvidenceError::SealedInventory)),
        "inspection cannot read an incomplete or emptied index"
    );
    Ok(())
}
