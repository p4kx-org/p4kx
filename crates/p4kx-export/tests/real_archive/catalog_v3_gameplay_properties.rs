// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

#[path = "catalog_v3_gameplay_transformations.rs"]
pub(crate) mod transformations;

use p4kx_export::Kind;
use serde_json::Value;

use super::TestResult;

pub(super) fn check(row: &Value) -> TestResult<()> {
    if super::field(row, Kind::GameplayProperties, "sourceType")?.as_str()
        != Some("CraftingGameplayPropertyDef")
    {
        return Err("the definition belongs to the original GameplayProperty source family".into());
    }
    text(
        row.get("propertyName")
            .ok_or("required propertyName carrier")?,
    )?;
    text(row.get("unitFormat").ok_or("required unitFormat carrier")?)?;
    transformations::check(
        row.get("displayTransformation")
            .ok_or("required transformation carrier")?,
    )?;
    if let Some(values) = super::source_value(
        row.get("nameOverrides")
            .ok_or("required overrides carrier")?,
    )? {
        for entry in values.as_array().ok_or("ordered override array")? {
            if let Some(entry) = super::source_value(entry)? {
                override_fields(entry)?;
            }
        }
    }
    Ok(())
}

fn text(carrier: &Value) -> TestResult<()> {
    if let Some(value) = super::source_value(carrier)?
        && !value.is_string()
    {
        return Err("public text reference must be a string".into());
    }
    Ok(())
}

fn source_payload(value: &Value) -> TestResult<Option<&Value>> {
    let source_type = super::present(value.get("sourceType").ok_or("source type carrier")?)?;
    if !source_type.is_string() {
        return Err("original concrete source type is a string".into());
    }
    super::source_value(value.get("payload").ok_or("typed payload carrier")?)
}

fn override_fields(value: &Value) -> TestResult<()> {
    if let Some(fields) = source_payload(value)? {
        text(
            fields
                .get("propertyName")
                .ok_or("override propertyName carrier")?,
        )?;
        if let Some(condition) =
            super::source_value(fields.get("condition").ok_or("condition carrier")?)?
        {
            condition_fields(condition)?;
        }
    }
    Ok(())
}

fn condition_fields(value: &Value) -> TestResult<()> {
    if let Some(payload) = source_payload(value)? {
        match payload.get("type").and_then(Value::as_str) {
            Some("emptyBase") => {}
            Some("itemType") => {
                for name in ["matchItemTypes", "matchSubTypes"] {
                    let carrier = payload.get(name).ok_or("required ordered Enum array")?;
                    if let Some(labels) = super::source_value(carrier)? {
                        for label in labels.as_array().ok_or("original Enum array")? {
                            text(label)?;
                        }
                    }
                }
            }
            _ => return Err("closed property-condition payload tag".into()),
        }
    }
    Ok(())
}
