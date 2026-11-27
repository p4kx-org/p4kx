// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use p4kx_export::catalog_queries::{
    OriginalValueErrorFacts, ResourceNumber, SourceArenaOutput, SourceDataType, SourceField,
    SourceNode, SourcePayload, SourceText, SourceWeakTarget,
};
use p4kx_extraction::{FieldOrigin, SourceInventory, SourceValueState, Step};

#[path = "catalog_source_arena_fixture.rs"]
mod fixture;

pub(super) use fixture::{OWNER, database};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

pub(super) fn verify(arena: &SourceArenaOutput<'_>, inventory: &SourceInventory) -> TestResult {
    assert_eq!(
        arena.roots().len(),
        11,
        "all original root declarations are available"
    );
    assert_eq!(
        arena
            .roots()
            .first()
            .ok_or("first original root field")?
            .name()
            .value(),
        Some(&"numbers"),
        "root declaration order"
    );
    assert!(
        arena
            .roots()
            .iter()
            .all(|field| field.name().value() != Some(&"absent")),
        "an absent declaration remains distinct from a null pointer"
    );
    verify_numbers(arena)?;
    verify_texts(arena)?;
    verify_sequence(arena)?;
    verify_weak_targets(arena, inventory)?;
    verify_error(arena, inventory)?;
    verify_graph(arena, inventory)?;
    assert_eq!(
        root(arena, "nullPointer")?.value().state(),
        SourceValueState::Null,
        "the null original pointer has its own state"
    );
    let SourcePayload::Guid { value } = present(root(arena, "identity")?)? else {
        return Err("the original GUID payload".into());
    };
    assert_eq!(
        value.get().to_string(),
        crate::commodity_v3_fixture::guid(OWNER),
        "the typed GUID needs no serialization"
    );
    assert!(
        matches!(
            present(root(arena, "enabled")?)?,
            SourcePayload::Boolean { value: true }
        ),
        "the typed Boolean value is directly available"
    );
    Ok(())
}

fn verify_numbers(arena: &SourceArenaOutput<'_>) -> TestResult {
    let numbers = root(arena, "numbers")?;
    let SourcePayload::Number {
        value: ResourceNumber::Int64 { value },
    } = present(member(arena, numbers, "signed")?)?
    else {
        return Err("the exact source Int64".into());
    };
    assert_eq!(
        value.get(),
        i64::MIN,
        "the signed value retains all 64 bits"
    );
    let SourcePayload::Number {
        value: ResourceNumber::UInt64 { value },
    } = present(member(arena, numbers, "unsigned")?)?
    else {
        return Err("the exact source UInt64".into());
    };
    assert_eq!(
        value.get(),
        u64::MAX,
        "the unsigned value retains all 64 bits"
    );
    let SourcePayload::Number {
        value: ResourceNumber::Single(value),
    } = present(member(arena, numbers, "single")?)?
    else {
        return Err("the exact source Single".into());
    };
    assert_eq!(
        value.raw_bits(),
        0.1_f32.to_bits(),
        "the original IEEE bits remain accessible"
    );
    assert_eq!(
        value.finite(),
        Some(0.1_f32),
        "the finite numeric projection remains available"
    );
    Ok(())
}

fn verify_texts(arena: &SourceArenaOutput<'_>) -> TestResult {
    let texts = root(arena, "texts")?;
    let SourcePayload::String { value } = present(member(arena, texts, "unitFormat")?)? else {
        return Err("the authored public string".into());
    };
    assert!(
        matches!(value, SourceText::Public { .. }),
        "the public text domain survives"
    );
    assert_eq!(
        value.as_str(),
        "%.2f RPM",
        "the original public text is borrowed"
    );
    let SourcePayload::Locale { value } = present(member(arena, texts, "note")?)? else {
        return Err("the authored protected locale key".into());
    };
    assert!(
        matches!(value, SourceText::GameText { .. }),
        "the protected text domain survives"
    );
    assert_eq!(
        value.as_str(),
        "@original_note",
        "the authored locale key is retained"
    );
    Ok(())
}

fn verify_sequence(arena: &SourceArenaOutput<'_>) -> TestResult {
    let SourcePayload::Array { elements } = present(root(arena, "sequence")?)? else {
        return Err("the source reference array".into());
    };
    let [first, null, dangling, repeated] = elements.as_slice() else {
        return Err("all four original array positions".into());
    };
    assert_ne!(
        first.value_index(),
        repeated.value_index(),
        "repeated values retain distinct occurrences"
    );
    let SourcePayload::RecordReference { value } =
        present(arena.value(*first).ok_or("first owner-local element")?)?
    else {
        return Err("the resolved target header".into());
    };
    assert_eq!(
        value.target_record_index().value(),
        Some(&0),
        "the original physical target is retained"
    );
    assert_eq!(
        value.guid().value().map(ToString::to_string),
        Some(crate::commodity_v3_fixture::guid(fixture::TARGET)),
        "the source target GUID"
    );
    assert_eq!(
        arena.value(*null).ok_or("null element")?.value().state(),
        SourceValueState::Null,
        "the null slot stays present in the array"
    );
    let dangling = arena.value(*dangling).ok_or("dangling element")?;
    assert_eq!(
        dangling.value().state(),
        SourceValueState::Unreadable,
        "the dangling Reference retains its unreadable source state"
    );
    let Some(OriginalValueErrorFacts::DanglingReference { guid }) = dangling.original_error()
    else {
        return Err("the original dangling-reference error".into());
    };
    assert_eq!(
        guid.value().map(|value| value.get().to_string()),
        Some(crate::commodity_v3_fixture::guid(fixture::DANGLING)),
        "the failed original GUID"
    );
    let SourcePayload::Array { elements } = present(root(arena, "empty")?)? else {
        return Err("the declared empty array".into());
    };
    assert!(
        elements.is_empty(),
        "an empty array remains distinct from an absent field"
    );
    Ok(())
}

fn verify_weak_targets(arena: &SourceArenaOutput<'_>, inventory: &SourceInventory) -> TestResult {
    let SourcePayload::WeakReference { value } = present(root(arena, "weakOwned")?)? else {
        return Err("the original owned-instance weak pointer".into());
    };
    let SourceWeakTarget::Instance {
        source_type,
        location,
    } = value.target()
    else {
        return Err("the actual owned-instance target".into());
    };
    assert_eq!(
        source_type.value(),
        Some(&"Owned"),
        "the target's concrete source type"
    );
    assert_eq!(
        location.value().map(AsRef::<str>::as_ref),
        Some("owned"),
        "the complete original instance location"
    );
    assert_eq!(
        value.instance_steps(),
        Some([Step::Field("owned")].as_slice()),
        "the original typed route is directly available"
    );
    origin(location.origin(), inventory);
    origin(value.record_guid().origin(), inventory);
    let SourcePayload::WeakReference { value } = present(root(arena, "weakRecord")?)? else {
        return Err("the original Record weak pointer".into());
    };
    assert!(
        matches!(value.target(), SourceWeakTarget::Record { .. }),
        "a Record target is explicit"
    );
    assert!(
        value.instance_steps().is_none(),
        "the Record target has no owned-instance route"
    );
    assert_eq!(
        value.target_record_index().value(),
        Some(&0),
        "the original Record target position"
    );
    Ok(())
}

fn verify_error(arena: &SourceArenaOutput<'_>, inventory: &SourceInventory) -> TestResult {
    let broken = root(arena, "broken")?;
    assert_eq!(
        broken.value().state(),
        SourceValueState::Unreadable,
        "the malformed Boolean has an explicit unreadable source state"
    );
    assert!(
        !broken.value().origin().source_set_complete(),
        "the unreadable value stays incomplete"
    );
    let Some(OriginalValueErrorFacts::InvalidBool { byte }) = broken.original_error() else {
        return Err("the exact original Boolean error".into());
    };
    assert_eq!(
        byte.value(),
        Some(&2),
        "the original parser byte is accessible"
    );
    origin(byte.origin(), inventory);
    Ok(())
}

fn verify_graph(arena: &SourceArenaOutput<'_>, inventory: &SourceInventory) -> TestResult {
    let mut pending: Vec<_> = arena.roots().iter().map(SourceField::value).collect();
    let mut visited = BTreeSet::new();
    while let Some(reference) = pending.pop() {
        assert!(
            visited.insert(reference.value_index()),
            "each original occurrence has one owner"
        );
        let node = arena
            .value(reference)
            .ok_or("an in-scope owner-local value link")?;
        origin(node.value().origin(), inventory);
        match node.value().value() {
            Some(SourcePayload::Struct {
                source_type,
                fields,
            }) => {
                origin(source_type.origin(), inventory);
                for field in fields {
                    verify_field(field, inventory);
                    pending.push(field.value());
                }
            }
            Some(SourcePayload::Array { elements }) => pending.extend(elements),
            _ => {}
        }
    }
    assert_eq!(
        visited.len(),
        arena.values().len(),
        "typed links reach every original occurrence"
    );
    for field in arena.roots() {
        verify_field(field, inventory);
    }
    assert!(
        matches!(
            arena
                .roots()
                .first()
                .ok_or("first original root field")?
                .declaration()
                .data_type()
                .value(),
            Some(SourceDataType::Class)
        ),
        "the original field storage kind is directly available"
    );
    assert_eq!(
        arena
            .roots()
            .first()
            .ok_or("first original root field")?
            .declaration()
            .is_array()
            .value(),
        Some(&false),
        "the original scalar declaration remains sourced"
    );
    Ok(())
}

fn verify_field(field: &SourceField<'_>, inventory: &SourceInventory) {
    origin(field.name().origin(), inventory);
    origin(field.declaration().data_type().origin(), inventory);
    origin(field.declaration().is_array().origin(), inventory);
    origin(field.declaration().target().origin(), inventory);
}

fn origin(origin: &FieldOrigin, inventory: &SourceInventory) {
    for reference in origin.source_uses_refs() {
        assert!(
            inventory.iter_uses().any(|used| used.id() == reference),
            "every retained source-use reference resolves within the same snapshot"
        );
    }
}

fn root<'arena, 'source>(
    arena: &'arena SourceArenaOutput<'source>,
    name: &str,
) -> TestResult<&'arena SourceNode<'source>> {
    field(arena, arena.roots(), name)
}

fn member<'arena, 'source>(
    arena: &'arena SourceArenaOutput<'source>,
    node: &SourceNode<'source>,
    name: &str,
) -> TestResult<&'arena SourceNode<'source>> {
    let SourcePayload::Struct { fields, .. } = present(node)? else {
        return Err("the original nested structure".into());
    };
    field(arena, fields, name)
}

fn field<'arena, 'source>(
    arena: &'arena SourceArenaOutput<'source>,
    fields: &[SourceField<'source>],
    name: &str,
) -> TestResult<&'arena SourceNode<'source>> {
    let selected = fields
        .iter()
        .find(|field| field.name().value() == Some(&name))
        .ok_or("the original fixture field")?;
    arena
        .value(selected.value())
        .ok_or_else(|| "the field's owner-local value".into())
}

fn present<'node, 'source>(
    node: &'node SourceNode<'source>,
) -> TestResult<&'node SourcePayload<'source>> {
    node.value()
        .value()
        .ok_or_else(|| "a present original source value".into())
}
