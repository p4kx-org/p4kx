// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, TestResult};

const FIELD: &str = "defaultCargoContainers.oneSCU";
const NAMESPACE: &str = "dataforgeCargoSlot";

fn assert_slot_uses(
    catalog: &Value,
    carrier: &Value,
    ordinal: usize,
    target: u32,
    field: &str,
    role: &str,
) -> TestResult {
    let uses = contributions(catalog, carrier)?;
    let mut actual = false;
    for used in uses {
        match used.pointer("/location/namespace").and_then(Value::as_str) {
            Some(NAMESPACE) => {
                assert_eq!(
                    common::at(used, "/location/indices")?,
                    &json!(["0", ordinal.to_string()]),
                    "the tuple is physical Record0 plus local field ordinal in the selected cargo struct"
                );
                let label = common::at(used, "/location/field")?
                    .as_str()
                    .ok_or("logical source field")?;
                assert!(
                    label == FIELD || label == "defaultCargoContainers.oneSCU.$fieldName",
                    "no unrelated source field selects this slot"
                );
                actual |= label == field && common::at(used, "/role")?.as_str() == Some(role);
            }
            Some("dataforgeRecordHeader") => {
                assert_eq!(
                    common::at(used, "/location/indices")?,
                    &json!([target.to_string()]),
                    "any contributing target-header fact belongs to this target only"
                );
            }
            actual => assert_eq!(
                actual,
                Some(NAMESPACE),
                "cargo origin needs its original local declaration coordinate"
            ),
        }
    }
    assert!(
        actual,
        "the immediate datum retains its actual {role} at local cargo ordinal{ordinal}"
    );
    Ok(())
}

fn assert_header_use(
    catalog: &Value,
    carrier: &Value,
    index: u32,
    field: &str,
    role: &str,
) -> TestResult {
    let uses = contributions(catalog, carrier)?;
    assert!(
        !uses.is_empty(),
        "a target header keeps its own actual contribution"
    );
    for used in uses {
        assert_eq!(
            common::at(used, "/location/namespace")?,
            &json!("dataforgeRecordHeader"),
            "target headers never inherit the caller's cargo-slot mode"
        );
        assert_eq!(
            common::at(used, "/location/indices")?,
            &json!([index.to_string()]),
            "the target is addressed by its own physical Record position"
        );
        assert_eq!(
            common::at(used, "/location/field")?,
            &json!(field),
            "the original header field is explicit"
        );
        assert_eq!(
            common::at(used, "/role")?,
            &json!(role),
            "the actual header selector/conversion is preserved"
        );
    }
    Ok(())
}

fn contributions<'a>(catalog: &'a Value, carrier: &Value) -> TestResult<Vec<&'a Value>> {
    assert_eq!(
        common::at(carrier, "/containsEncryptedSource")?,
        &json!(false),
        "this synthetic DCB has a complete known-plain original input"
    );
    assert_eq!(
        common::at(carrier, "/sourceSetComplete")?,
        &json!(true),
        "the exact small source tuple is fully captured"
    );
    let references = common::at(carrier, "/sourceUsesRefs")?
        .as_array()
        .ok_or("immediate source-use refs")?;
    assert!(
        !references.is_empty(),
        "the immediate field cites its actual input, not only a row marker"
    );
    let inventory = common::at(catalog, "/provenance/uses")?
        .as_array()
        .ok_or("use inventory")?;
    let mut result = Vec::new();
    for reference in references {
        let mut found = inventory
            .iter()
            .filter(|used| used.get("id") == Some(reference));
        let used = found.next().ok_or("closed original use")?;
        assert!(
            found.next().is_none(),
            "one use ID identifies one actual inventory row"
        );
        assert_eq!(
            common::at(used, "/scope")?,
            &json!("payload"),
            "these source and header facts come from the DCB payload"
        );
        assert!(
            matches!(
                common::at(used, "/role")?.as_str(),
                Some("payload" | "selector" | "conversion")
            ),
            "attempted/untouched targets are not field contributors"
        );
        assert_dcb_source(catalog, used)?;
        result.push(used);
    }
    Ok(result)
}

fn assert_dcb_source(catalog: &Value, used: &Value) -> TestResult {
    let sources = common::at(catalog, "/provenance/sources")?
        .as_array()
        .ok_or("physical input inventory")?;
    let source = sources
        .iter()
        .find(|source| source.get("id") == used.get("source"))
        .ok_or("closed physical DCB input")?;
    assert_eq!(
        common::at(source, "/kind")?,
        &json!("archiveEntry"),
        "no whole-Archive or fabricated disk input stands in for the DCB"
    );
    assert_eq!(
        common::at(source, "/path/value")?,
        &json!("Data\\Game2.dcb"),
        "both references come from the one actual original DCB Entry"
    );
    Ok(())
}

pub(crate) fn assert_slots(catalog: &Value, slots: &[Value]) -> TestResult {
    for (ordinal, (slot, target)) in slots.iter().zip([1_u32, 2_u32]).enumerate() {
        assert_slot_origin(catalog, slot, ordinal, target)?;
    }
    Ok(())
}

fn assert_slot_origin(catalog: &Value, slot: &Value, ordinal: usize, target: u32) -> TestResult {
    assert_slot_uses(catalog, slot, ordinal, target, FIELD, "payload")?;
    assert_slot_uses(
        catalog,
        common::at(slot, "/value/slot")?,
        ordinal,
        target,
        "defaultCargoContainers.oneSCU.$fieldName",
        "selector",
    )?;
    let reference = common::at(slot, "/value/entityClass")?;
    assert_slot_uses(catalog, reference, ordinal, target, FIELD, "payload")?;
    assert_slot_uses(
        catalog,
        common::at(reference, "/value/guid")?,
        ordinal,
        target,
        FIELD,
        "payload",
    )?;
    assert_header_use(
        catalog,
        common::at(reference, "/value/targetRecordIndex")?,
        target,
        "recordIndex",
        "selector",
    )?;
    assert_header_use(
        catalog,
        common::at(reference, "/value/recordPath")?,
        target,
        "fileName",
        "conversion",
    )?;
    Ok(())
}
