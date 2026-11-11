// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Original resource tag inputs and their shared lineage in location queries.

use std::collections::BTreeSet;

use p4kx_extraction::{DataForge, Value as OriginalValue};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};
use serde_json::{Value, json};

use crate::catalog_tags_fixture::{self as fixture, TestResult, at};
use crate::flow_owner_graph;
use crate::health_origin_assertions;
use crate::loot_v3_graph as raw;
use crate::template_canonical_graph::Template;
use crate::template_fixture_fields;

use super::{CALLERS, TAGS, TEMPLATE};

#[test]
fn a_location_query_retains_each_tag_operand_and_the_exact_shared_lineage() -> TestResult {
    let database = spawn_database();
    let bytes = database.build();
    let forge = DataForge::open(&bytes)?;
    assert_spawn_source(&forge)?;
    let (_, catalog) = fixture::diagnostic(&database)?;
    let template = Template::new(&catalog, &fixture::guid(TEMPLATE))?;
    let property = crate::template_fixture_context::named(&template, "Location")?;
    let location = crate::template_fixture_context::value(&template, property)?;
    let resource_tags = template.field(location, "resourceTags")?;
    let callers = template_fixture_fields::elements(&template, resource_tags)?;
    let mut ordinals = callers
        .iter()
        .map(|caller| flow_owner_graph::ordinal(caller))
        .collect::<TestResult<BTreeSet<_>>>()?;
    assert_eq!(
        ordinals.len(),
        CALLERS,
        "the query selects every original caller occurrence"
    );
    ordinals.insert(flow_owner_graph::ordinal(resource_tags)?);
    let result =
        crate::template_fixture_context::context(&template, "spawnPools", "source", location)?;
    let query = at(result, "/result/query")?;
    assert_spawn_origin(&catalog, query, &ordinals)?;

    let guid = raw::present(query)?;
    let content = raw::array(&catalog, "contractLocationQueries")?
        .iter()
        .find(|row| row.get("guid") == Some(guid))
        .ok_or("actual registered location query")?;
    let expected = vec!["tag"; TAGS].join(".");
    assert_eq!(
        raw::array(content, "resourceTags")?,
        &vec![json!(expected); CALLERS],
        "the content registration preserves the original ordered operands, including repeats"
    );
    Ok(())
}

fn spawn_database() -> DataForgeBuilder {
    let tag = super::tag_guid(TAGS - 1);
    let location = TestInstance::new("MissionPropertyValue_Location")
        .value("resourceTags", fixture::references(&[tag; CALLERS]));
    let property = TestInstance::new("MissionProperty")
        .value("missionVariableName", TestValue::text("Location"))
        .value("value", TestValue::Struct(location));
    super::tag_chain()
        .structure(TestStruct::new("ContractTemplate").array(
            "contractProperties",
            TestType::Class("MissionProperty".to_owned()),
        ))
        .structure(
            TestStruct::new("MissionProperty")
                .field("missionVariableName", TestType::String)
                .field(
                    "value",
                    TestType::StrongPointer("MissionPropertyValue".to_owned()),
                ),
        )
        .structure(TestStruct::new("MissionPropertyValue"))
        .structure(
            TestStruct::new("MissionPropertyValue_Location")
                .parent("MissionPropertyValue")
                .array("resourceTags", TestType::ReferenceTo("Tag".to_owned())),
        )
        .record(
            TestRecord::new(
                TEMPLATE,
                "ContractTemplate",
                "ContractTemplate.SharedLocation",
                "libs/foundry/records/contracts/contracttemplates/sharedlocation.xml",
            )
            .value(
                "contractProperties",
                TestValue::Array(vec![TestValue::Struct(property)]),
            ),
        )
}

fn assert_spawn_source(forge: &DataForge<'_>) -> TestResult {
    let tree = forge
        .record(u32::try_from(TAGS + 1)?)
        .ok_or("actual Template owner")?
        .tree()?;
    assert_eq!(
        tree.error_count(),
        0,
        "the original Template has a readable location property"
    );
    let [OriginalValue::Struct(property)] =
        super::original_array(tree.field("contractProperties"))?
    else {
        return Err("one original location property".into());
    };
    let Some(OriginalValue::Struct(location)) = property.field("value") else {
        return Err("actual location source struct".into());
    };
    let tags = super::original_array(location.field("resourceTags"))?;
    assert_eq!(
        tags.len(),
        CALLERS,
        "all original resourceTag reference slots are present"
    );
    for tag in tags {
        super::assert_edge(tag, super::tag_guid(TAGS - 1), TAGS - 1)?;
    }
    Ok(())
}

fn assert_spawn_origin(catalog: &Value, query: &Value, ordinals: &BTreeSet<u64>) -> TestResult {
    assert_eq!(
        at(query, "/sourceSetComplete")?,
        &json!(true),
        "the original registration and all operand reads are complete"
    );
    assert_eq!(
        at(query, "/containsEncryptedSource")?,
        &json!(false),
        "all participating original inputs are unencrypted"
    );
    let tail = fixture::row(catalog, "projections/tagPaths", u64::try_from(TAGS - 1)?)?;
    let shared = super::use_refs(at(tail, "/path")?)?;
    let used = super::use_refs(query)?;
    assert!(
        shared.is_subset(&used),
        "the query retains all actual selected Tag lineage uses"
    );
    let mut actual = BTreeSet::new();
    for used in health_origin_assertions::uses(catalog, query)? {
        if used.pointer("/location/namespace") != Some(&json!("dataforgeValue"))
            || used.pointer("/location/field") != Some(&json!("$value"))
            || used.get("role") != Some(&json!("selector"))
        {
            continue;
        }
        let indices = raw::array(at(used, "/location")?, "indices")?;
        let [record, ordinal] = indices else {
            return Err("actual DCB value use".into());
        };
        assert_eq!(
            record,
            &json!((TAGS + 1).to_string()),
            "the operand belongs to the selected physical Template owner"
        );
        actual.insert(
            ordinal
                .as_str()
                .ok_or("actual caller ordinal")?
                .parse::<u64>()?,
        );
    }
    assert_eq!(
        &actual, ordinals,
        "the query retains the original resourceTags array and each distinct reference slot"
    );
    Ok(())
}
