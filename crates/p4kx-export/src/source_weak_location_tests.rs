// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    DataType, InputScope, Location, RecordTree, SourceInventory, SourceLocation, SourceUseRole,
    SourceValueState, SourcedValue, Value,
};
use serde::Serialize;

use crate::build::original_input::test_support::{self, TestResult};
use crate::catalog_read::original::{OriginalRecordScope, OriginalValueEvent};

use super::super::reference::SourceWeakTarget;
use super::super::value::SourcePayload;
use super::weak_location_fixture as fixture;

struct Retained<'source> {
    value: SourcePayload<'source>,
    ordinal: u64,
}

#[test]
fn repeated_weak_locations_share_the_actual_original_location_bytes() -> TestResult {
    test_support::with_input(&fixture::database(), |forge, input| {
        let record = forge.record(0).ok_or("actual original owner")?;
        let tree = record.tree()?;
        assert!(tree.is_from(&record), "actual tree and owner pairing");
        let original = original(&tree)?;
        let scope = input.record_scope(&record, &tree)?;
        let retained = retain(&scope)?;
        assert_eq!(
            retained.len(),
            fixture::REFERENCES,
            "every genuine weak occurrence is retained"
        );
        let inventory = scope.sources().freeze()?;
        for retained in &retained {
            let location = location(&retained.value)?;
            let text: &str = location.value().ok_or("actual retained location")?.as_ref();
            assert_eq!(
                text,
                original.as_str(),
                "complete original generated location"
            );
            roles(location, &inventory, retained.ordinal);
        }
        for retained in &retained {
            let location = location(&retained.value)?;
            let text: &str = location.value().ok_or("actual retained location")?.as_ref();
            assert!(
                std::ptr::eq::<str>(text, original.as_str()),
                "retention must share the genuine original immutable bytes"
            );
        }
        Ok(())
    })
}

#[test]
fn retained_weak_locations_survive_dropping_the_original_tree_and_scope() -> TestResult {
    test_support::with_input(&fixture::database(), |forge, input| {
        let record = forge.record(0).ok_or("actual original owner")?;
        let tree = record.tree()?;
        assert!(
            tree.is_from(&record),
            "actual original owner before capture"
        );
        original(&tree)?;
        let retained = {
            let scope = input.record_scope(&record, &tree)?;
            retain(&scope)?
        };
        drop(tree);
        let expected = fixture::location();
        for retained in &retained {
            let cell = location(&retained.value)?;
            let text: &str = cell.value().ok_or("owned retained location")?.as_ref();
            assert_eq!(
                text, expected,
                "complete original text outlives the temporary tree"
            );
            let json = serde_json::to_value(cell)?;
            assert_eq!(
                json.get("value").and_then(serde_json::Value::as_str),
                Some(expected.as_str()),
                "unchanged string wire after source-tree drop"
            );
        }
        Ok(())
    })
}

fn original<'tree, 'source>(
    tree: &'tree RecordTree<'source>,
) -> TestResult<&'tree Location<'source>> {
    assert_eq!(
        tree.error_count(),
        0,
        "all original weak references resolve"
    );
    assert_eq!(
        tree.fields().len(),
        fixture::REFERENCES + 1,
        "one owned target and every original caller"
    );
    let mut selected: Option<&'tree Location<'source>> = None;
    for field in tree.fields().iter().skip(1) {
        assert_eq!(
            field.storage().data_type(),
            DataType::WeakPointer,
            "actual weak storage"
        );
        let Value::Reference(reference) = field.value() else {
            return Err("actual resolved original weak reference".into());
        };
        let admitted = reference.original_target();
        assert_eq!(
            admitted.record_index(),
            0,
            "actual instance belongs to the original owner"
        );
        assert!(
            admitted.record_type().is_none(),
            "an instance never becomes the owner Record header"
        );
        assert_eq!(
            admitted.instance_positions(),
            Some([0_u64].as_slice()),
            "actual original field position"
        );
        let location = reference
            .location()
            .ok_or("actual shared original location")?;
        assert_eq!(
            location.as_str(),
            fixture::location(),
            "the original long location really exists"
        );
        assert_eq!(
            reference.type_name(),
            "SharedTarget",
            "actual concrete instance type"
        );
        if let Some(first) = selected {
            assert!(
                std::ptr::eq(first, location),
                "canonical weak references already share their real Location allocation"
            );
        } else {
            selected = Some(location);
        }
    }
    selected.ok_or_else(|| "actual original instance target".into())
}

fn retain<'source>(scope: &OriginalRecordScope<'_, 'source>) -> TestResult<Vec<Retained<'source>>> {
    let mut walk = scope.walk(None);
    let mut retained = Vec::new();
    let mut visited = 0;
    while let Some(event) = walk.next_event()? {
        let OriginalValueEvent::Value(site) = event else {
            continue;
        };
        visited += 1;
        if site.field().storage().data_type() == DataType::WeakPointer {
            assert!(
                matches!(site.value(), Value::Reference(_)),
                "real same-site resolved value"
            );
            retained.push(Retained {
                value: super::read(site.fact().reference_observation()?, scope.sources())?,
                ordinal: site.ordinal(),
            });
        }
    }
    assert_eq!(
        visited,
        fixture::REFERENCES + fixture::OWNED_VALUES,
        "actual owned target, marker and every caller were visited"
    );
    for (index, retained) in retained.iter().enumerate() {
        assert_eq!(
            retained.ordinal,
            u64::try_from(
                index
                    .checked_add(fixture::OWNED_VALUES)
                    .ok_or("source ordinal")?
            )?,
            "each original weak caller retains its distinct full-record ordinal"
        );
    }
    Ok(retained)
}

fn location<'value, 'source>(
    value: &'value SourcePayload<'source>,
) -> TestResult<&'value SourcedValue<impl AsRef<str> + Serialize + 'source>> {
    let SourcePayload::WeakReference { value } = value else {
        return Err("retained weak payload".into());
    };
    let SourceWeakTarget::Instance {
        source_type,
        location,
    } = &value.target
    else {
        return Err("retained original instance".into());
    };
    assert_eq!(
        source_type.value().copied(),
        Some("SharedTarget"),
        "actual instance type survives capture"
    );
    assert_eq!(
        location.state(),
        SourceValueState::Present,
        "the generated location is present"
    );
    assert_eq!(
        location.origin().contains_encrypted_source(),
        Some(false),
        "actual plain source marker"
    );
    assert!(
        location.origin().source_set_complete(),
        "same-site source set remains complete"
    );
    Ok(location)
}

fn roles<T>(cell: &SourcedValue<T>, inventory: &SourceInventory, ordinal: u64) {
    let refs = cell.origin().source_uses_refs();
    assert_eq!(
        refs.len(),
        2,
        "one actual caller Selector and one actual name Conversion"
    );
    let expected = [
        (
            SourceUseRole::Selector,
            SourceLocation::Subresource {
                namespace: "dataforgeValue".to_owned(),
                indices: vec![0, ordinal],
                field: "$value".to_owned(),
            },
        ),
        (
            SourceUseRole::Conversion,
            SourceLocation::Subresource {
                namespace: "dataforgeTraversal".to_owned(),
                indices: vec![0, 0],
                field: format!("{}.$fieldName", fixture::location()),
            },
        ),
    ];
    for (role, location) in expected {
        let count = refs
            .iter()
            .filter_map(|reference| {
                inventory
                    .uses()
                    .iter()
                    .find(|source| source.id() == reference)
            })
            .filter(|source| {
                source.scope() == InputScope::Payload
                    && source.role() == role
                    && source.location() == &location
            })
            .count();
        assert_eq!(
            count, 1,
            "exact existing role/coordinate before storage-sharing assertion"
        );
    }
}
