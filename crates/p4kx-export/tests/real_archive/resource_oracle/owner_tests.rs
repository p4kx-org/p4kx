// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::Value;

use super::{Failure, OverrideEntryTarget, Result, Written};

#[test]
fn every_original_resource_owner_requires_exactly_one_row() -> Result<()> {
    let written = Written::new(OverrideEntryTarget::Concrete)?;
    let mut changed = written.catalog.clone();
    let rows_path = "/definitions/resourceTypes/rows";
    let rows = changed
        .pointer_mut(rows_path)
        .and_then(Value::as_array_mut)
        .ok_or_else(|| Failure::check("synthetic-resource-rows"))?;
    let removed = rows.remove(0);
    assert_eq!(
        written.verify(&changed).err().map(|error| error.stage),
        Some("missing-resource-owner"),
        "remaining valid rows cannot hide a missing original owner"
    );
    let rows = changed
        .pointer_mut(rows_path)
        .and_then(Value::as_array_mut)
        .ok_or_else(|| Failure::check("synthetic-resource-rows"))?;
    rows.push(removed.clone());
    rows.push(removed);
    assert_eq!(
        written.verify(&changed).err().map(|error| error.stage),
        Some("duplicate-or-outside-resource-owner"),
        "authored evidence cannot give two rows the same physical owner"
    );
    Ok(())
}
