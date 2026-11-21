// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::equipment::EquipmentSnapshot;
use serde::Serialize;
use serde_json::Value;

use super::TestResult;

pub(super) fn parity<Row: Serialize, Dependencies>(
    dataset: &str,
    snapshot: &EquipmentSnapshot<'_, Row, Dependencies>,
    catalog: &Value,
) -> TestResult<()> {
    let expected = catalog
        .pointer(dataset)
        .ok_or("canonical physical component dataset")?;
    assert_eq!(
        serde_json::to_value(snapshot.availability())?,
        expected["availability"],
        "SDK and Catalog agree on source-family availability for {dataset}"
    );
    let mut expected = expected["rows"].clone();
    let mut actual = serde_json::to_value(snapshot.rows())?;
    omit_document_local_ids(&mut expected);
    omit_document_local_ids(&mut actual);
    assert_eq!(
        actual, expected,
        "canonical values and origin flags for {dataset}"
    );
    Ok(())
}

fn omit_document_local_ids(value: &mut Value) {
    match value {
        Value::Array(values) => values.iter_mut().for_each(omit_document_local_ids),
        Value::Object(object) => {
            object.remove("sourceUsesRefs");
            object.values_mut().for_each(omit_document_local_ids);
        }
        _ => {}
    }
}
