// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogPublicationMode, Filter};
use p4kx_testkit::DataForgeBuilder;
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{DcbOrigin, TestResult};
use crate::quality_v3_fixture_schema as schema;
use crate::quality_v3_fixture_values as values;
use crate::quality_v3_origin_support::{self as origins, Domain, Use};
use crate::quality_v3_support as support;

const NEGATIVE_RESOURCES: u16 = 1500;
const LATE_RESOURCE: u128 = 0x9000;

fn database() -> DataForgeBuilder {
    let mut database = schema::schema().record(values::quantization(
        values::QUANTIZATION,
        "Quantizer",
        "quantizer",
        values::bands(vec![values::band(0, 9, 3)]),
    ));
    for ordinal in 0..NEGATIVE_RESOURCES {
        database = database.record(values::resource(
            u128::from(ordinal) + 0x1000,
            &format!("Negative{ordinal}"),
            vec![values::property(None)],
        ));
    }
    database.record(values::resource(
        LATE_RESOURCE,
        "LatePositive",
        vec![values::property(Some(values::QUANTIZATION))],
    ))
}

#[test]
fn quality_v3_optional_inventory_cut_keeps_the_complete_inverse_and_late_encrypted_value()
-> TestResult {
    let filter = Filter::new("*/quantizer");
    let (_, catalog) = support::build(
        &database(),
        DcbOrigin::Encrypted,
        CatalogPublicationMode::Normal,
        Some(&filter),
    )?;
    assert_eq!(
        support::rows(&catalog, "definitions/resourceTypes")?.len(),
        0,
        "relationship dependencies do not publish filtered resource definitions"
    );
    let inverse = support::row(&catalog, support::INVERSE, 0)?;
    let list = support::at(inverse, "/resourceTypeGuids")?;
    support::marker(list, Some(true))?;
    assert_eq!(
        support::at(list, "/sourceSetComplete")?,
        &json!(false),
        "optional proof limits remain explicit without truncating the actual relationship scan"
    );
    assert_opaque_encrypted_witness(&catalog, list)?;
    assert_late_member(&catalog, list)?;
    assert_eq!(
        support::at(&catalog, "/exportHealth/status")?,
        &json!("complete"),
        "optional provenance-coordinate retention is independent of complete source reads"
    );
    let health = support::kind_health(&catalog, "qualityQuantizations")?;
    assert_eq!(
        support::at(health, "/sourceRecords/dependencies")?,
        &json!(u64::from(NEGATIVE_RESOURCES) + 1)
    );
    Ok(())
}

fn assert_late_member(catalog: &Value, list: &Value) -> TestResult {
    let members = support::at(list, "/value")?
        .as_array()
        .ok_or("inverse members")?;
    assert_eq!(
        members.len(),
        1,
        "a positive declaration after the inventory cut is still found"
    );
    let member = members.first().ok_or("late resource")?;
    support::state(member, "present")?;
    support::marker(member, Some(true))?;
    assert_eq!(
        support::at(member, "/value")?,
        &json!(support::guid(LATE_RESOURCE)),
        "the late positive declaration retains its actual original resource GUID"
    );
    assert_eq!(
        support::at(member, "/sourceSetComplete")?,
        &json!(true),
        "the shared inventory cut does not starve a late value's independent positive proof"
    );
    let used = origins::uses(catalog, member)?;
    let late = u64::from(NEGATIVE_RESOURCES) + 1;
    assert!(
        used.contains(&Use::new(Domain::Header, late, "guid", "payload")),
        "the actual late resource GUID supplies its own nested value"
    );
    assert!(
        used.contains(&Use::new(
            Domain::Body,
            late,
            "properties[0].qualityQuantization.qualityQuantizationRecord",
            "selector"
        )),
        "the child retains the actual positive relationship declaration"
    );
    Ok(())
}

fn assert_opaque_encrypted_witness(catalog: &Value, carrier: &Value) -> TestResult {
    let references = support::at(carrier, "/sourceUsesRefs")?
        .as_array()
        .ok_or("bounded references")?;
    let inventory = support::at(catalog, "/provenance/uses")?
        .as_array()
        .ok_or("use inventory")?;
    let witness = inventory
        .iter()
        .find(|used| {
            used.pointer("/location/type").and_then(Value::as_str) == Some("opaque")
                && used.get("id").is_some_and(|id| references.contains(id))
        })
        .ok_or("a retained original capture-limit witness")?;
    assert_eq!(
        support::at(witness, "/location/reason")?,
        &json!("captureLimit"),
        "the opaque witness records the actual optional capture boundary"
    );
    assert_eq!(
        support::at(witness, "/effectiveEncryption")?,
        &json!("encrypted"),
        "the retained known encrypted input survives the coordinate cut"
    );
    assert_eq!(
        support::at(witness, "/scope")?,
        &json!("payload"),
        "the witness covers the actual DCB payload domain"
    );
    assert_eq!(
        support::at(witness, "/role")?,
        &json!("selector"),
        "the bounded inverse inventory contributes as a selector"
    );
    Ok(())
}
