// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogError, build_catalog};
use p4kx_extraction::{DataForge, DataType, Value as Raw};
use p4kx_testkit::DataForgeBuilder;
use serde_json::{Value, json};

use super::fixture::{self, Case, MODIFIER, WRONG_TARGET};
use super::{MODIFIERS, ROWS, component, configuration, diagnostic, source_ordinal};
use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;

#[test]
fn hacking_components_empty_target_rejects_an_unrelated_class_even_without_elements() -> TestResult
{
    let source = fixture::database(Case::WrongEmptyTarget);
    let (ordinal, target) = original_empty_target(&source, WRONG_TARGET)?;
    let (_folder, archive) = common::open(&source, DcbOrigin::Encrypted)?;
    assert!(
        matches!(
            build_catalog(&archive, &common::options()),
            Err(CatalogError::IncompleteExport(_))
        ),
        "normal publication cannot report an unrelated declared Class array as known empty"
    );
    let catalog = diagnostic(&archive)?;
    let row = proof::rows(&catalog, ROWS)?
        .get(2)
        .ok_or("actual empty modifier row")?;
    let config = configuration(row, 3, MODIFIERS, "removableChip", true)?;
    let values = proof::cell(config, "values", "unsupported")?;
    assert_eq!(
        values.get("sourceSetComplete"),
        Some(&json!(true)),
        "the declaration proving the rejection is fully readable"
    );
    assert_eq!(
        values.get("containsEncryptedSource"),
        Some(&json!(true)),
        "the rejection comes from the actual encrypted declaration"
    );
    check_selection(&catalog, values, ordinal, target)
}

#[test]
fn hacking_components_empty_target_keeps_the_original_empty_class_and_its_exact_selectors()
-> TestResult {
    let source = fixture::database(Case::Valid);
    let (ordinal, target) = original_empty_target(&source, MODIFIER)?;
    let (_folder, archive) = common::open(&source, DcbOrigin::Plain)?;
    let catalog: Value =
        serde_json::from_slice(build_catalog(&archive, &common::options())?.json())?;
    let row = proof::rows(&catalog, ROWS)?
        .get(2)
        .ok_or("actual empty modifier row")?;
    let config = configuration(row, 3, MODIFIERS, "removableChip", false)?;
    let values = proof::present(config, "values", &json!([]), Some(false))?;
    check_selection(&catalog, values, ordinal, target)
}

fn original_empty_target(source: &DataForgeBuilder, expected: &str) -> TestResult<(u64, u16)> {
    let bytes = source.build();
    let forge = DataForge::open(&bytes)?;
    let tree = forge.record(0).ok_or("physical chip owner")?.tree()?;
    let owner = component(&tree, 3)?;
    let field = owner
        .fields()
        .iter()
        .find(|field| field.name() == "values")
        .ok_or("original field")?;
    assert_eq!(
        field.storage().data_type(),
        DataType::Class,
        "both controls store a genuine Class declaration"
    );
    assert!(
        field.storage().is_array(),
        "both original declarations are arrays"
    );
    assert!(
        matches!(field.value(), Raw::Array(values) if values.is_empty()),
        "both arrays decode successfully and contain no original element"
    );
    let index = field.storage().target();
    let structure = forge
        .structs()
        .nth(usize::from(index))
        .ok_or("declared target definition")?;
    assert_eq!(
        structure.name()?,
        expected,
        "the original declaration names its actual target"
    );
    Ok((source_ordinal(&tree, field.value())?, index))
}

fn check_selection(catalog: &Value, cell: &Value, ordinal: u64, target: u16) -> TestResult {
    let uses = proof::uses(catalog, cell)?;
    let field_slot = json!(["0", ordinal.to_string()]);
    let structure_slot = json!([target.to_string()]);
    assert!(
        uses.iter()
            .any(|used| selector(used, "dataforgeValue", "$target", &field_slot)),
        "the exact source declaration's target word is a selector"
    );
    assert!(
        uses.iter()
            .any(|used| selector(used, "dataforgeStruct", "name", &structure_slot)),
        "the actual target definition name contributes as a selector"
    );
    Ok(())
}

fn selector(used: &Value, namespace: &str, field: &str, indices: &Value) -> bool {
    used.get("role").and_then(Value::as_str) == Some("selector")
        && used.pointer("/location/namespace").and_then(Value::as_str) == Some(namespace)
        && used.pointer("/location/field").and_then(Value::as_str) == Some(field)
        && used.pointer("/location/indices") == Some(indices)
}
