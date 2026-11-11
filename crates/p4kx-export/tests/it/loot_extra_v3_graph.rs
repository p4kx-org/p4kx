// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{InputScope, SourceInventory, SourceLocation, SourceUseRole};
use serde_json::{Value, json};

use crate::loot_extra_v3_fixture::{self as fixture, Checked};

pub(crate) struct Graph<'catalog> {
    row: &'catalog Value,
    values: &'catalog [Value],
}

impl<'catalog> Graph<'catalog> {
    pub(crate) fn new(catalog: &'catalog Value, count: usize) -> Checked<Self> {
        let table = catalog
            .pointer("/definitions/lootPoolFilters")
            .ok_or("canonical Loot definitions table")?;
        assert_eq!(
            table.get("availability"),
            Some(&json!("available")),
            "the genuine selected source table exists"
        );
        let rows = table
            .get("rows")
            .and_then(Value::as_array)
            .ok_or("canonical physical rows")?;
        assert_eq!(rows.len(), 1, "one physical Record yields one row");
        let row = rows.first().ok_or("the actual canonical row")?;
        assert_eq!(
            present(row.get("guid").ok_or("original header GUID")?)?,
            &json!(fixture::GUID_TEXT),
            "original header identity"
        );
        let values = row
            .get("values")
            .and_then(Value::as_array)
            .ok_or("complete owner-local value arena")?;
        assert_eq!(
            values.len(),
            count,
            "all independently enumerated original occurrences survive"
        );
        Ok(Self { row, values })
    }

    pub(crate) fn fields(&self) -> Checked<&'catalog [Value]> {
        array(self.row, "fields")
    }

    pub(crate) fn follow(&self, reference: &Value) -> Checked<&'catalog Value> {
        let text = reference
            .get("valueIndex")
            .and_then(Value::as_str)
            .ok_or("decimal arena reference")?;
        let index: u64 = text.parse()?;
        assert_eq!(
            text,
            index.to_string(),
            "arena reference is canonical DecimalU64"
        );
        self.values
            .get(usize::try_from(index)?)
            .ok_or_else(|| "arena reference must be in this row".into())
    }

    pub(crate) fn field(
        &self,
        fields: &[Value],
        index: usize,
        name: &str,
        data_type: &str,
        is_array: bool,
    ) -> Checked<&'catalog Value> {
        let field = fields.get(index).ok_or("original ordered field")?;
        assert_eq!(
            present(field.get("name").ok_or("sourced field name")?)?,
            &json!(name),
            "original declaration order/name"
        );
        let declaration = field
            .get("declaration")
            .ok_or("decoder-minted declaration")?;
        assert_eq!(
            present(declaration.get("dataType").ok_or("declared element type")?)?,
            &json!(data_type),
            "exact declared storage type"
        );
        assert_eq!(
            present(declaration.get("isArray").ok_or("declared array flag")?)?,
            &json!(is_array),
            "actual array declaration"
        );
        present(declaration.get("target").ok_or("original target word")?)?;
        self.follow(field.get("value").ok_or("ordered field value reference")?)
    }
}

pub(crate) fn array<'value>(owner: &'value Value, name: &str) -> Checked<&'value [Value]> {
    let values = owner
        .get(name)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("required {name} array"))?;
    Ok(values)
}

pub(crate) fn cell(cell: &Value, state: &str) {
    assert_eq!(
        cell.get("state"),
        Some(&json!(state)),
        "exact immediate state"
    );
    assert!(
        cell.get("value").is_some(),
        "the immediate nullable value key is required"
    );
    assert!(
        cell.get("containsEncryptedSource").is_some(),
        "the immediate encryption marker key is required"
    );
    assert!(
        cell.get("sourceSetComplete").is_some(),
        "the immediate completeness key is required"
    );
    assert!(
        cell.get("sourceUsesRefs")
            .and_then(Value::as_array)
            .is_some(),
        "the immediate references are materialized"
    );
}

pub(crate) fn present(cell: &Value) -> Checked<&Value> {
    self::cell(cell, "present");
    assert_eq!(
        cell.get("containsEncryptedSource"),
        Some(&json!(false)),
        "this original stored Entry is plain"
    );
    assert_eq!(
        cell.get("sourceSetComplete"),
        Some(&json!(true)),
        "tiny complete original fact capture"
    );
    Ok(cell.get("value").ok_or("required immediate value")?)
}

pub(crate) fn payload<'node>(node: &'node Value, kind: &str) -> Checked<&'node Value> {
    assert_eq!(
        node.get("originalError"),
        Some(&Value::Null),
        "non-error applicability is explicitly null"
    );
    let payload = present(node.get("value").ok_or("immediate original value")?)?;
    assert_eq!(
        payload.get("type"),
        Some(&json!(kind)),
        "exact retained source variant"
    );
    Ok(payload)
}

pub(crate) fn scalar(node: &Value, kind: &str, value: &Value) -> Checked {
    assert_eq!(
        payload(node, kind)?.get("value"),
        Some(value),
        "exact original scalar, without another sourced wrapper"
    );
    Ok(())
}

pub(crate) fn witness(
    cell: &Value,
    inventory: &SourceInventory,
    ordinal: u64,
    token: &str,
) -> Checked {
    let refs = cell
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("immediate source references")?;
    let mut found = false;
    for reference in refs {
        let id = reference.as_str().ok_or("source reference string")?;
        let observed = inventory
            .uses()
            .iter()
            .find(|observed| observed.id().as_str() == id)
            .ok_or("actual immutable inventory use")?;
        assert!(
            matches!(
                observed.role(),
                SourceUseRole::Payload | SourceUseRole::Selector | SourceUseRole::Conversion
            ),
            "only actual contributing uses belong to the immediate cell"
        );
        if observed.role() == SourceUseRole::Payload {
            let SourceLocation::Subresource {
                namespace,
                indices,
                field,
            } = observed.location()
            else {
                return Err("stored fact must use its actual original Loot occurrence".into());
            };
            assert_eq!(
                namespace, "dataforgeValue",
                "no unrelated source namespace supplies this raw fact"
            );
            assert_eq!(
                indices.as_slice(),
                [0, ordinal],
                "the actual original position supplies this fact"
            );
            assert_eq!(
                field, token,
                "only this exact original token supplies retained payload"
            );
            assert_eq!(
                observed.scope(),
                InputScope::Payload,
                "original DataForge Entry payload, not directory metadata"
            );
            found = true;
        }
    }
    assert!(
        found,
        "a nonempty exact original ordinal/token witness is required"
    );
    Ok(())
}
