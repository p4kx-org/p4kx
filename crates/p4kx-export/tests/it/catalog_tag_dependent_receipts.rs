// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use p4kx_extraction::{DataForge, Value as OriginalValue};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};
use serde_json::{Value, json};

use crate::catalog_tags_fixture::{self as fixture, TestResult, at};
use crate::flow_owner_graph;
use crate::health_origin_assertions;
use crate::loot_v3_graph as raw;
use crate::mission_template_graph;
use crate::template_canonical_graph::{self as graph, BrokerContext, Context, Template};
use crate::template_fixture_fields;

#[path = "catalog_tag_dependent_location.rs"]
mod location;

const TAGS: usize = 80;
const CALLERS: usize = 24;
const FIRST_TAG: u128 = 0xa100_0000_0000_4000_8000_0000_0000_0000;
const TEMPLATE: u128 = 0xa200_0000_0000_4000_8000_0000_0000_0001;
const BROKER: u128 = 0xa200_0000_0000_4000_8000_0000_0000_0002;

#[test]
fn repeated_selected_tag_paths_keep_every_physical_template_and_broker_caller() -> TestResult {
    let database = database();
    let bytes = database.build();
    let forge = DataForge::open(&bytes)?;
    assert_original(&forge)?;
    let (_, catalog) = fixture::diagnostic(&database)?;
    let tail = fixture::row(&catalog, "projections/tagPaths", u64::try_from(TAGS - 1)?)?;
    let path = at(tail, "/path")?;
    let text = vec!["tag"; TAGS].join(".");
    assert_eq!(
        raw::present(path)?,
        &json!(text),
        "the full selected physical lineage"
    );
    let shared = use_refs(path)?;
    assert_eq!(
        shared.len(),
        TAGS * 5 + 3,
        "one local name, edge and three headers per Tag plus the database headers"
    );
    let template = Template::new(&catalog, &fixture::guid(TEMPLATE))?;
    let comms = graph::root_field(&template.owner, "contractEndCommsNotifications")?;
    let template_tags = template_fixture_fields::elements(&template, comms)?
        .into_iter()
        .map(|node| template.field(node, "communicationTags"))
        .collect::<TestResult<Vec<_>>>()?;
    assert_context(
        &catalog,
        &template,
        &template_tags,
        TAGS + 1,
        &shared,
        &text,
    )?;
    let broker = BrokerContext::new(&catalog, &fixture::guid(BROKER))?;
    let broker_tags = template_fixture_fields::elements(&broker, broker.root("missionTags")?)?;
    assert_context(&catalog, &broker, &broker_tags, TAGS + 2, &shared, &text)?;
    Ok(())
}

fn tag_chain() -> DataForgeBuilder {
    let mut result = fixture::schema();
    for index in 0..TAGS {
        let next = [tag_guid(index + 1)];
        let children = if index + 1 == TAGS { &[][..] } else { &next };
        result = result.record(fixture::tag(
            tag_guid(index),
            "tag",
            fixture::references(children),
        ));
    }
    result.record(fixture::roots(fixture::references(&[FIRST_TAG])))
}

fn database() -> DataForgeBuilder {
    let result = tag_chain();
    let tag = tag_guid(TAGS - 1);
    let comms = TestInstance::new("ContractEndCommsNotification")
        .value("communicationTags", TestValue::Reference(tag));
    result
        .structure(TestStruct::new("ContractTemplate").array(
            "contractEndCommsNotifications",
            TestType::Class("ContractEndCommsNotification".to_owned()),
        ))
        .structure(
            TestStruct::new("ContractEndCommsNotification")
                .field("communicationTags", TestType::ReferenceTo("Tag".to_owned())),
        )
        .structure(
            TestStruct::new("MissionBrokerEntry")
                .array("missionTags", TestType::ReferenceTo("Tag".to_owned())),
        )
        .record(
            TestRecord::new(
                TEMPLATE,
                "ContractTemplate",
                "ContractTemplate.SharedTag",
                "libs/foundry/records/contracts/contracttemplates/sharedtag.xml",
            )
            .value(
                "contractEndCommsNotifications",
                TestValue::Array(
                    (0..CALLERS)
                        .map(|_| TestValue::Struct(comms.clone()))
                        .collect(),
                ),
            ),
        )
        .record(
            TestRecord::new(
                BROKER,
                "MissionBrokerEntry",
                "MissionBrokerEntry.SharedTag",
                "libs/foundry/records/missionbroker/sharedtag.xml",
            )
            .value("missionTags", fixture::references(&[tag; CALLERS])),
        )
}

const fn tag_guid(index: usize) -> u128 {
    FIRST_TAG + index as u128
}

fn assert_original(forge: &DataForge<'_>) -> TestResult {
    assert_eq!(
        forge.records().count(),
        TAGS + 3,
        "all physical Records exist"
    );
    for index in 0..TAGS {
        let record = forge
            .record(u32::try_from(index)?)
            .ok_or("actual Tag Record")?;
        assert_eq!(
            record.guid().to_string(),
            fixture::guid(tag_guid(index)),
            "the chain member has its fixture GUID at index {index}"
        );
        let tree = record.tree()?;
        assert_eq!(
            tree.error_count(),
            0,
            "the original Tag at index {index} is readable"
        );
        assert!(
            matches!(tree.field("tagName"), Some(OriginalValue::String("tag"))),
            "the chain member contributes its stored name"
        );
        let children = original_array(tree.field("children"))?;
        if index + 1 == TAGS {
            assert_eq!(
                children,
                &[],
                "the terminal original Tag has no child references"
            );
        } else {
            let [child] = children else {
                return Err("one actual selected child".into());
            };
            assert_edge(child, tag_guid(index + 1), index + 1)?;
        }
    }
    let database = forge
        .record(u32::try_from(TAGS)?)
        .ok_or("actual TagDatabase")?
        .tree()?;
    let [root] = original_array(database.field("tags"))? else {
        return Err("one actual declared root".into());
    };
    assert_edge(root, FIRST_TAG, 0)?;
    assert_callers(forge)
}

fn assert_callers(forge: &DataForge<'_>) -> TestResult {
    let template = forge
        .record(u32::try_from(TAGS + 1)?)
        .ok_or("original Template")?
        .tree()?;
    assert_eq!(
        template.guid().to_string(),
        fixture::guid(TEMPLATE),
        "the Template uses its original owner GUID"
    );
    assert_eq!(
        template.error_count(),
        0,
        "the Template caller array is readable"
    );
    let comms = original_array(template.field("contractEndCommsNotifications"))?;
    assert_eq!(
        comms.len(),
        CALLERS,
        "the Template stores every repeated caller"
    );
    let tail = tag_guid(TAGS - 1);
    for item in comms {
        let OriginalValue::Struct(comms) = item else {
            return Err("actual comms item".into());
        };
        let reference = comms
            .field("communicationTags")
            .ok_or("authored Template caller")?;
        assert_edge(reference, tail, TAGS - 1)?;
    }
    let broker = forge
        .record(u32::try_from(TAGS + 2)?)
        .ok_or("original Broker")?
        .tree()?;
    assert_eq!(
        broker.guid().to_string(),
        fixture::guid(BROKER),
        "the Broker uses its original owner GUID"
    );
    assert_eq!(
        broker.error_count(),
        0,
        "the Broker caller array is readable"
    );
    let tags = original_array(broker.field("missionTags"))?;
    assert_eq!(
        tags.len(),
        CALLERS,
        "the Broker stores every repeated caller"
    );
    for reference in tags {
        assert_edge(reference, tail, TAGS - 1)?;
    }
    Ok(())
}

fn original_array<'borrow, 'source>(
    value: Option<&'borrow OriginalValue<'source>>,
) -> TestResult<&'borrow [OriginalValue<'source>]> {
    let Some(OriginalValue::Array(values)) = value else {
        return Err("actual authored array".into());
    };
    Ok(values.as_ref())
}

fn assert_edge(value: &OriginalValue<'_>, guid: u128, record: usize) -> TestResult {
    let OriginalValue::Reference(reference) = value else {
        return Err("actual authored reference".into());
    };
    assert_eq!(
        reference.guid().to_string(),
        fixture::guid(guid),
        "the selected reference keeps its original target GUID"
    );
    assert_eq!(
        reference.record_index(),
        u32::try_from(record)?,
        "the selected reference points to the physical target Record"
    );
    Ok(())
}

fn use_refs(cell: &Value) -> TestResult<BTreeSet<&str>> {
    let refs = raw::array(cell, "sourceUsesRefs")?;
    let uses = refs
        .iter()
        .map(|value| value.as_str().ok_or_else(|| "original use id".into()))
        .collect::<TestResult<BTreeSet<_>>>()?;
    assert_eq!(uses.len(), refs.len(), "wire use references are distinct");
    Ok(uses)
}

fn assert_context(
    catalog: &Value,
    owner: &dyn Context<'_>,
    values: &[&Value],
    record: usize,
    shared: &BTreeSet<&str>,
    text: &str,
) -> TestResult {
    let rows = raw::array(owner.projection(), "tagPaths")?;
    assert_eq!(
        rows.len(),
        CALLERS,
        "each contextual caller keeps one output row"
    );
    assert_eq!(
        values.len(),
        CALLERS,
        "each contextual caller still links to one physical source value"
    );
    let mut caller_uses = BTreeSet::new();
    for (row, value) in rows.iter().zip(values) {
        let (path, ordinal) = assert_caller_path(owner, row, value, text)?;
        let actual = use_refs(path)?;
        assert!(
            shared.is_subset(&actual),
            "each cell retains the whole chosen original Tag lineage"
        );
        let caller = assert_caller(catalog, path, record, ordinal)?;
        assert!(
            caller_uses.insert(caller),
            "each physical caller has its own exact use"
        );
        let mut extra = actual.difference(shared);
        assert_eq!(
            extra.next(),
            Some(&caller),
            "only this actual caller augments the shared evidence"
        );
        assert!(
            extra.next().is_none(),
            "no other original caller contributes"
        );
    }
    Ok(())
}

fn assert_caller_path<'row>(
    owner: &dyn Context<'_>,
    row: &'row Value,
    value: &Value,
    text: &str,
) -> TestResult<(&'row Value, u64)> {
    let ordinal = flow_owner_graph::ordinal(value)?;
    let reference = owner.owner().reference(ordinal)?;
    assert_eq!(
        row.get("source"),
        Some(&reference),
        "the same actual original caller slot"
    );
    mission_template_graph::reference(
        value,
        &fixture::guid(tag_guid(TAGS - 1)),
        Some(u32::try_from(TAGS - 1)?),
    )?;
    assert_eq!(
        at(row, "/interpretation")?,
        &json!("path"),
        "the valid Tag reference resolves to a selected path"
    );
    let path = at(row, "/path")?;
    assert_eq!(
        raw::present(path)?,
        &json!(text),
        "each caller renders the complete selected chain"
    );
    assert_eq!(
        at(path, "/sourceSetComplete")?,
        &json!(true),
        "all contributing sources were read"
    );
    assert_eq!(
        at(path, "/containsEncryptedSource")?,
        &json!(false),
        "the original chain and caller are proven unencrypted"
    );
    Ok((path, ordinal))
}

fn assert_caller<'catalog>(
    catalog: &'catalog Value,
    path: &Value,
    record: usize,
    ordinal: u64,
) -> TestResult<&'catalog str> {
    let matches: Vec<_> = health_origin_assertions::uses(catalog, path)?
        .into_iter()
        .filter(|used| used.pointer("/location/namespace") == Some(&json!("dataforgeValue")))
        .collect();
    let [caller] = matches.as_slice() else {
        return Err("one actual caller use".into());
    };
    assert_eq!(
        at(caller, "/location/indices")?,
        &json!([record.to_string(), ordinal.to_string()]),
        "the caller use identifies the original owner and exact value ordinal"
    );
    assert_eq!(
        at(caller, "/location/field")?,
        &json!("$value"),
        "the selector reads the original reference value"
    );
    assert_eq!(
        at(caller, "/role")?,
        &json!("selector"),
        "the caller reference selects the shared path"
    );
    assert_eq!(
        at(caller, "/scope")?,
        &json!("payload"),
        "the caller selector reads its physical payload"
    );
    at(caller, "/id")?
        .as_str()
        .ok_or_else(|| "actual caller use identity".into())
}
