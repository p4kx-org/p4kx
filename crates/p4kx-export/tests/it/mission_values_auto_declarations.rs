// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, DataForgeError, DataType, Field, StructValue, Value as Raw};
use serde_json::{Value, json};

use super::{OPTIONS, TestResult, mission_property, value};
use crate::health_origin_assertions as proof;
use crate::mission_values_fixture::{self as fixture, AutoLayout, Owner};

const SEARCH: &str = "searchOptions";
const BASE: &str = "SpawnClosetSearchOptionsBase";

#[derive(Clone, Copy)]
enum Declaration {
    Matches,
    Unrelated,
    Unreadable,
}

impl Declaration {
    const fn layout(self) -> AutoLayout {
        match self {
            Self::Matches | Self::Unreadable => AutoLayout::Authored,
            Self::Unrelated => AutoLayout::WrongSearchTarget,
        }
    }

    const fn code(self) -> Option<&'static str> {
        match self {
            Self::Matches => None,
            Self::Unrelated => Some("unsupportedValue"),
            Self::Unreadable => Some("unreadableSource"),
        }
    }

    const fn role(self) -> &'static str {
        match self {
            Self::Matches | Self::Unrelated => "selector",
            Self::Unreadable => "attempted",
        }
    }
}

#[test]
fn auto_spawns_distinguishes_an_unreadable_original_declaration() -> TestResult {
    for owner in [Owner::Template, Owner::Broker, Owner::Generator] {
        for case in [
            Declaration::Matches,
            Declaration::Unrelated,
            Declaration::Unreadable,
        ] {
            check(owner, case)?;
        }
    }
    Ok(())
}

fn check(owner: Owner, case: Declaration) -> TestResult {
    let layout = case.layout();
    let property = fixture::property("NominalEvidence", value(layout));
    let mut bytes = fixture::database_with(vec![property], owner, layout).build();
    if matches!(case, Declaration::Unreadable) {
        proof::declaration_fixture::corrupt_definition_name(&mut bytes, BASE)?;
    }
    let (target, ordinal) = original(&bytes, owner, case)?;
    let catalog = proof::declaration_fixture::diagnostic_bytes(bytes)?;
    if let Some(code) = case.code() {
        let field = diagnostic_field(&catalog, code)?;
        check_field(&catalog, field, ordinal, target, case)?;
    } else {
        assert_eq!(
            proof::at(&catalog, "/exportHealth/status")?,
            "complete",
            "readable matching declarations preserve successful publication"
        );
    }
    let indices = json!([target.to_string()]);
    assert!(
        proof::rows(&catalog, "/provenance/uses")?
            .iter()
            .any(|row| { is_use(row, "dataforgeStruct", "name", &indices, case.role()) }),
        "the actual declaration name is retained with its original lookup role"
    );
    proof::closure(&catalog)
}

fn original(bytes: &[u8], owner: Owner, case: Declaration) -> TestResult<(u16, usize)> {
    let forge = DataForge::open(bytes)?;
    let record = forge.record(0).ok_or("physical owner")?;
    let tree = record.tree()?;
    assert!(
        tree.is_from(&record) && tree.error_count() == 0,
        "the owning Record and its value still decode"
    );
    let field = mission_property(&tree, owner)?
        .fields()
        .iter()
        .find(|field| field.name() == SEARCH)
        .ok_or("original search pointer declaration")?;
    let storage = field.storage();
    assert!(
        storage.data_type() == DataType::StrongPointer
            && !storage.is_array()
            && matches!(field.value(), Raw::Struct(source) if source.type_name() == OPTIONS),
        "the scalar pointer and its concrete original value are readable"
    );
    let name = forge
        .struct_at(u32::from(storage.target()))
        .ok_or("physical target Struct")?
        .name();
    match case {
        Declaration::Matches => assert_eq!(name?, BASE, "correct authored declaration name"),
        Declaration::Unrelated => assert_eq!(name?, "Definition", "different readable name"),
        Declaration::Unreadable => assert!(
            matches!(name, Err(DataForgeError::InvalidString { .. })),
            "the actual nominal target name is unreadable independently of its payload"
        ),
    }
    Ok((storage.target(), ordinal(tree.root(), field.value())?))
}

fn ordinal<'source>(root: &StructValue<'source>, selected: &Raw<'source>) -> TestResult<usize> {
    let mut pending: Vec<_> = root.fields().iter().rev().map(Field::value).collect();
    for ordinal in 0_usize.. {
        let value = pending.pop().ok_or("actual original field occurrence")?;
        if std::ptr::eq(value, selected) {
            return Ok(ordinal);
        }
        match value {
            Raw::Struct(source) => pending.extend(source.fields().iter().rev().map(Field::value)),
            Raw::Array(elements) => pending.extend(elements.iter().rev()),
            _ => {}
        }
    }
    Err("actual original field occurrence".into())
}

fn diagnostic_field<'catalog>(catalog: &'catalog Value, code: &str) -> TestResult<&'catalog Value> {
    for diagnostic in proof::rows(catalog, "/exportHealth/diagnostics")? {
        if diagnostic.get("code").and_then(Value::as_str) != Some(code) {
            continue;
        }
        for example in proof::rows(diagnostic, "/examples")? {
            if proof::example_text(catalog, example, "sourceType")?.and_then(Value::as_str)
                == Some(super::VALUE)
                && proof::example_text(catalog, example, "field")?.and_then(Value::as_str)
                    == Some(SEARCH)
            {
                return proof::at(example, "/field");
            }
        }
    }
    Err(format!("missing exact {code} diagnostic for the original search declaration").into())
}

fn check_field(
    catalog: &Value,
    field: &Value,
    ordinal: usize,
    target: u16,
    case: Declaration,
) -> TestResult {
    let uses = proof::uses(catalog, field)?;
    let caller = json!(["0", ordinal.to_string()]);
    for member in ["$fieldName", "$dataType", "$isArray", "$target"] {
        assert!(
            uses.iter()
                .any(|row| is_use(row, "dataforgeValue", member, &caller, "selector")),
            "diagnostic cites original storage selector {member} at the actual occurrence"
        );
    }
    let target = json!([target.to_string()]);
    assert_eq!(
        uses.iter()
            .any(|row| is_use(row, "dataforgeStruct", "name", &target, "selector")),
        matches!(case, Declaration::Unrelated),
        "only a readable name may contribute to the diagnostic"
    );
    assert!(
        uses.iter()
            .all(|row| row.get("role").and_then(Value::as_str) != Some("attempted")),
        "an attempted read is inventory evidence without a false contributing claim"
    );
    Ok(())
}

fn is_use(row: &Value, namespace: &str, field: &str, indices: &Value, role: &str) -> bool {
    row.get("role").and_then(Value::as_str) == Some(role)
        && row.pointer("/location/namespace").and_then(Value::as_str) == Some(namespace)
        && row.pointer("/location/field").and_then(Value::as_str) == Some(field)
        && row.pointer("/location/indices") == Some(indices)
}
