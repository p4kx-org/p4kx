// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use p4kx_export::Filter;
use p4kx_extraction::{DataForge, DataForgeEntry, Value as Raw};
use p4kx_testkit::DataForgeBuilder;
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::quality_origin_wide_fixture;
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
fn quality_v3_complete_inventory_keeps_every_negative_probe_and_late_member() -> TestResult {
    let filter = Filter::new("*/quantizer");
    let database = database();
    let (_directory, archive) = common::open(&database, DcbOrigin::Encrypted)?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let expected = original(&forge)?;
    let mut options = common::options();
    options.filter = Some(&filter);
    let built = p4kx_export::build_catalog_from(&archive, &entry, &options)?;
    let catalog: Value = serde_json::from_slice(built.json())?;
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
        &json!(true),
        "all actual original relation decisions contribute"
    );
    let retained = quality_origin_wide_fixture::relationships(&catalog, list)?;
    assert_eq!(
        retained.len(),
        expected.len(),
        "every actual original relationship decision survives"
    );
    assert!(
        retained == expected,
        "exact physical negative and late positive sources"
    );
    assert_late_member(&catalog, list)?;
    assert_eq!(
        support::at(&catalog, "/exportHealth/status")?,
        &json!("complete"),
        "complete provenance and source reads are separately measured"
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
        "the later actual positive declaration is retained"
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
        "the late value retains its independent positive proof"
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

fn original(forge: &DataForge<'_>) -> TestResult<BTreeSet<(u32, usize)>> {
    let mut found = BTreeSet::new();
    for record in forge.records().skip(1) {
        let tree = record.tree()?;
        assert!(
            tree.is_from(&record),
            "each negative source probe uses its actual Resource Record"
        );
        let Some(Raw::Array(properties)) = tree.root().field("properties") else {
            return Err("original properties".into());
        };
        let [Raw::Struct(property)] = properties.as_ref() else {
            return Err("one actual property".into());
        };
        let Some(Raw::Struct(quantization)) = property.field("qualityQuantization") else {
            return Err("actual quantization wrapper".into());
        };
        let slot = quantization.field("qualityQuantizationRecord");
        if record.index() <= u32::from(NEGATIVE_RESOURCES) {
            assert_eq!(slot, Some(&Raw::Null), "negative authored relationship");
        } else {
            let Some(Raw::Reference(reference)) = slot else {
                return Err("actual late Reference".into());
            };
            assert_eq!(
                reference.record_index(),
                0,
                "the original late Reference selects the quantizer by physical index"
            );
        }
        found.insert((record.index(), 0));
    }
    assert_eq!(
        found.len(),
        usize::from(NEGATIVE_RESOURCES) + 1,
        "the evidence contains every negative probe and the one late match"
    );
    Ok(found)
}
