// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;
use std::error::Error;

use jsonschema::Validator;
use serde_json::Value;

use super::TestResult;
use super::catalog_schema_support::minimal_catalog;
use super::schema_navigation::schema_at;

const ORIGIN_FIELDS: [&str; 3] = [
    "containsEncryptedSource",
    "sourceUsesRefs",
    "sourceSetComplete",
];
const AUTHORED_PATHS: &[&str] = &[
    "/legacyLootTables/[]/recordIndex",
    "/legacyLootTables/[]/entries",
    "/legacyLootTables/[]/entries/value/[]",
    "/legacyLootTables/[]/entries/value/[]/value/weight",
    "/legacyLootTables/[]/entries/value/[]/value/archetypeGuid",
    "/legacyLootArchetypes/[]/excludedTagGuids/value/[]",
    "/legacyLootArchetypes/[]/entries/value/[]/value/weight",
    "/legacyLootArchetypes/[]/entries/value/[]/value/spawnWith/value/[]/value/mode",
    "/legacyLootArchetypes/[]/secondaryGroups/value/[]/value/entries/value/[]/value/weight",
    "/definitions/crafters/rows/[]/recordIndex",
    "/definitions/crafters/rows/[]/component",
    "/definitions/crafters/rows/[]/component/value/componentOrdinal",
    "/definitions/crafters/rows/[]/component/value/maxJobsInHistory",
    "/definitions/crafters/rows/[]/component/value/queues",
    "/items/[]/itemClass",
    "/ships/[]/mass",
    "/loadouts/[]/sourceRecordIndex",
];

#[test]
fn selected_authored_cells_require_source_markers() -> TestResult {
    let schema = p4kx_export::catalog_schema();
    let validator = jsonschema::validator_for(&schema)?;
    let mut catalog = minimal_catalog();
    assert!(
        validator.is_valid(&catalog),
        "the catalog fixture must validate before changing a selected cell"
    );
    for &path in AUTHORED_PATHS {
        assert_sourced_schema(schema_at(&schema, path)?, path)?;
        assert_unsourced_replacements_fail(&validator, &mut catalog, &path.replace("/[]", "/0"))?;
    }
    Ok(())
}

fn assert_sourced_schema(wrapper: &Value, path: &str) -> TestResult {
    let mut expected = BTreeSet::from(["state", "value"]);
    expected.extend(ORIGIN_FIELDS);
    let properties = wrapper["properties"]
        .as_object()
        .ok_or_else(|| format!("{path} must declare sourced value properties"))?;
    let required = wrapper["required"]
        .as_array()
        .ok_or_else(|| format!("{path} must declare required sourced value members"))?;
    let required_names = required
        .iter()
        .map(Value::as_str)
        .collect::<Option<BTreeSet<_>>>()
        .ok_or_else(|| format!("{path} must declare string property names"))?;
    assert_eq!(wrapper["type"], "object", "{path} must use an object");
    assert_eq!(
        properties
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>(),
        expected,
        "{path} must declare the exact sourced value properties"
    );
    assert_eq!(
        required.len(),
        expected.len(),
        "{path} required member count"
    );
    assert_eq!(
        required_names, expected,
        "{path} must require every sourced value property"
    );
    Ok(())
}

fn assert_unsourced_replacements_fail(
    validator: &Validator,
    catalog: &mut Value,
    path: &str,
) -> TestResult {
    let cell = fixture_cell(catalog, path)?;
    let raw_scalar = match cell.get("value") {
        Some(value @ (Value::Bool(_) | Value::Number(_) | Value::String(_))) => value.clone(),
        _ => Value::from(1),
    };
    let mut without_origin = cell.clone();
    let fields = without_origin
        .as_object_mut()
        .ok_or_else(|| format!("{path} must contain a sourced fixture cell"))?;
    for field in ORIGIN_FIELDS {
        fields
            .remove(field)
            .ok_or_else(|| format!("{path} is missing the fixture property {field}"))?;
    }
    let original = std::mem::replace(cell, without_origin);
    assert!(
        !validator.is_valid(catalog),
        "{path} must fail when all three origin properties are removed"
    );
    *fixture_cell(catalog, path)? = raw_scalar;
    assert!(
        !validator.is_valid(catalog),
        "{path} must fail when replaced by a bare scalar"
    );
    *fixture_cell(catalog, path)? = original;
    Ok(())
}

fn fixture_cell<'catalog>(
    catalog: &'catalog mut Value,
    path: &str,
) -> Result<&'catalog mut Value, Box<dyn Error>> {
    catalog
        .pointer_mut(path)
        .ok_or_else(|| format!("missing catalog fixture cell at {path}").into())
}
