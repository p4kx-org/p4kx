// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{RecordTree, Value as Raw};
use serde_json::Value;

use super::super::carriers::{Carriers, Uses};
use super::{Failure, Result, compare, require};

const GROUP_FIELDS: &[&str] = &[
    "displayName",
    "description",
    "defaultThumbnailPath",
    "defaultThumbnailPathSVG",
    "groups",
    "resources",
];

pub(super) fn definition<'catalog>(
    tree: &RecordTree<'_>,
    row: &'catalog Value,
    catalog: &'catalog Value,
    uses: &Uses<'catalog>,
) -> Result<()> {
    header(tree, row, uses)?;
    for field in tree.root().fields() {
        require(
            GROUP_FIELDS.contains(&field.name()),
            "new-group-source-field",
        )?;
    }
    let carriers = Carriers::new(row, uses, tree.record_index())?;
    for field in ["defaultThumbnailPath", "defaultThumbnailPathSVG"] {
        let carrier = row
            .get(field)
            .ok_or_else(|| Failure::check("group-thumbnail-carrier"))?;
        carriers.require(carrier, field)?;
        scalar(tree.field(field), carrier)?;
    }
    text(
        tree.field("displayName"),
        row.get("name"),
        catalog,
        "publicNames",
    )?;
    text(
        tree.field("description"),
        row.get("description"),
        catalog,
        "gameTexts",
    )?;
    Ok(())
}

fn scalar(raw: Option<&Raw<'_>>, carrier: &Value) -> Result<()> {
    let equal = match raw {
        None => compare::state(carrier, "absent"),
        Some(Raw::Null) => compare::state(carrier, "null"),
        Some(Raw::String(text)) => compare::present(carrier).and_then(Value::as_str) == Some(*text),
        Some(Raw::Error(_)) => compare::state(carrier, "unreadable"),
        _ => compare::state(carrier, "invalid"),
    };
    require(equal, "group-scalar-source-state")
}

fn text(
    raw: Option<&Raw<'_>>,
    carrier: Option<&Value>,
    catalog: &Value,
    domain: &str,
) -> Result<()> {
    let carrier = carrier.ok_or_else(|| Failure::check("group-text-carrier"))?;
    let Some(Raw::Locale(key)) = raw else {
        return scalar(raw, carrier);
    };
    let id = compare::present(carrier).ok_or_else(|| Failure::check("group-text-reference"))?;
    let table = catalog
        .get("texts")
        .and_then(|texts| texts.get(domain))
        .and_then(Value::as_array)
        .ok_or_else(|| Failure::check("group-text-domain"))?;
    let row = table
        .iter()
        .find(|row| row.pointer("/id/value") == Some(id))
        .ok_or_else(|| Failure::check("group-text-reference-closure"))?;
    require(
        row.pointer("/key/value").and_then(Value::as_str) == Some(*key),
        "group-original-text-key",
    )
}

pub(super) fn members<'catalog>(
    tree: &RecordTree<'_>,
    row: &'catalog Value,
    uses: &Uses<'catalog>,
) -> Result<()> {
    let carriers = Carriers::new(row, uses, tree.record_index())?;
    for field in ["groups", "resources"] {
        let carrier = row
            .get(field)
            .ok_or_else(|| Failure::check("group-array-carrier"))?;
        carriers.require(carrier, field)?;
        let Some(Raw::Array(original)) = tree.field(field) else {
            scalar(tree.field(field), carrier)?;
            continue;
        };
        let array = compare::present(carrier)
            .and_then(Value::as_array)
            .ok_or_else(|| Failure::check("group-original-array"))?;
        require(
            array.len() == original.len(),
            "group-original-array-cardinality",
        )?;
        for (ordinal, (raw, value)) in original.iter().zip(array).enumerate() {
            carriers.require(value, &format!("{field}[{ordinal}]"))?;
            member(raw, value)?;
        }
    }
    Ok(())
}

fn header(tree: &RecordTree<'_>, row: &Value, uses: &Uses<'_>) -> Result<()> {
    require(
        uses.record_index(row)? == tree.record_index(),
        "group-guid-source-owner",
    )?;
    let class_name = tree
        .name()
        .strip_prefix(tree.type_name())
        .and_then(|name| name.strip_prefix('.'))
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| tree.name());
    for (field, expected) in [
        ("guid", tree.guid().to_string()),
        ("recordName", tree.name().to_owned()),
        ("className", class_name.to_owned()),
        ("recordPath", tree.record_path().to_owned()),
        ("sourceType", tree.type_name().to_owned()),
    ] {
        require(
            row.get(field)
                .and_then(compare::present)
                .and_then(Value::as_str)
                == Some(expected.as_str()),
            "group-original-header",
        )?;
    }
    Ok(())
}

fn member(raw: &Raw<'_>, value: &Value) -> Result<()> {
    if matches!(raw, Raw::Null) {
        return require(compare::state(value, "null"), "group-original-null-slot");
    }
    let reference =
        compare::present(value).ok_or_else(|| Failure::check("group-reference-payload"))?;
    require(
        compare::reference(raw, reference),
        "group-original-reference",
    )
}
