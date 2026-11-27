// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{RecordText, RecordTree};
use serde_json::{Value, json};

use super::carriers::{Carriers, Uses};
use super::{Failure, Result, compare};

pub(super) fn check<'catalog>(
    tree: &RecordTree<'_>,
    row: &'catalog Value,
    uses: &Uses<'catalog>,
) -> Result<()> {
    if uses.record_index(row)? != tree.record_index() {
        return Err(Failure::check("header-guid-source-owner"));
    }
    let carriers = Carriers::new(row, uses, tree.record_index())?;
    let metadata = tree.metadata();
    for (field, original, expected) in [
        ("guid", "guid", json!(tree.guid().to_string())),
        ("recordIndex", "recordIndex", json!(tree.record_index())),
        ("sourceType", "sourceType", json!(tree.type_name())),
        ("recordPath", "fileName", json!(tree.record_path())),
        ("fileName", "fileName", json!(metadata.file_name())),
    ] {
        let carrier = field_value(row, field)?;
        carriers.require_header_at(carrier, tree.record_index(), original)?;
        if compare::present(carrier) != Some(&expected) {
            return Err(Failure::check("original-header-value"));
        }
    }
    raw_text(tree, row, &carriers, "recordName", metadata.name())?;
    raw_text(tree, row, &carriers, "metadataTag", metadata.metadata_tag())?;
    class_name(tree, row, &carriers)
}

fn raw_text(
    tree: &RecordTree<'_>,
    row: &Value,
    carriers: &Carriers<'_>,
    field: &str,
    expected: RecordText<'_>,
) -> Result<()> {
    let carrier = field_value(row, field)?;
    carriers.require_header_at(carrier, tree.record_index(), field)?;
    let equal = expected.fold(
        || compare::state(carrier, "absent"),
        || compare::state(carrier, "null"),
        |expected| compare::present(carrier).and_then(Value::as_str) == Some(expected),
    );
    if equal {
        Ok(())
    } else {
        Err(Failure::check("original-header-text-state"))
    }
}

fn class_name(tree: &RecordTree<'_>, row: &Value, carriers: &Carriers<'_>) -> Result<()> {
    let carrier = field_value(row, "className")?;
    carriers.require_header_at(carrier, tree.record_index(), "recordName")?;
    let declared = tree.metadata().name().fold(|| false, || false, |_| true);
    if declared {
        carriers.require_header_at(carrier, tree.record_index(), "sourceType")?;
    }
    let name = tree.name();
    let expected = name
        .strip_prefix(tree.type_name())
        .and_then(|name| name.strip_prefix('.'))
        .filter(|name| !name.is_empty())
        .unwrap_or(name);
    if compare::present(carrier).and_then(Value::as_str) == Some(expected) {
        Ok(())
    } else {
        Err(Failure::check("original-class-name-operation"))
    }
}

fn field_value<'a>(row: &'a Value, field: &str) -> Result<&'a Value> {
    row.get(field)
        .ok_or_else(|| Failure::check("canonical-header-field"))
}
