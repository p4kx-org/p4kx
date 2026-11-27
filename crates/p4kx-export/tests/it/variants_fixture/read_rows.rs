// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::Kind;
use serde_json::Value;

pub(crate) fn entry<'c>(catalog: &'c Value, kind: &str, class: &str) -> Result<&'c Value, String> {
    catalog[kind]
        .as_array()
        .and_then(|list| {
            list.iter().find(|entry| {
                crate::catalog_items::entity_field(entry, kind, "className").and_then(Value::as_str)
                    == Some(class)
            })
        })
        .ok_or_else(|| format!("{kind} {class} missing"))
}

pub(crate) fn base_of(catalog: &Value, kind: &str, class: &str) -> Result<Option<String>, String> {
    let row = entry(catalog, kind, class)?;
    let family = if kind == "items" {
        Kind::Items
    } else {
        Kind::Ships
    };
    let Some(base) = crate::catalog_variants::decision_reader::variant_target(row, family)
        .map_err(|error| error.to_string())?
    else {
        return Ok(None);
    };
    catalog[kind]
        .as_array()
        .and_then(|list| {
            list.iter().find(|entry| {
                crate::catalog_items::entity_field(entry, kind, "guid").and_then(Value::as_str)
                    == Some(base.guid)
                    && entry.pointer("/recordIndex/value").and_then(Value::as_u64)
                        == Some(u64::from(base.record_index))
            })
        })
        .and_then(|entry| {
            crate::catalog_items::entity_field(entry, kind, "className").and_then(Value::as_str)
        })
        .map(|name| Some(name.to_owned()))
        .ok_or_else(|| format!("variantOf of {class} resolves to no {kind} entry"))
}
