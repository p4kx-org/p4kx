// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_extraction::{DataForge, DataForgeHeaderField, SourceUseRole, SourceValue, Value};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

use super::{
    ATTACHABLE, ComponentKeyRead, capture_component_key, component_key, read_component_key,
};
use crate::build::original_input::test_support;
use crate::catalog_read::original::{
    OriginalErrorParameter, OriginalFlowFields, OriginalValuePart,
};

type TestResult = Result<(), Box<dyn Error>>;

const ITEM: &str = "EntityClassDefinition";
const COMPONENT: &str = "DataForgeComponentParams";
const DEFINITION: &str = "SItemDefinition";
const COMPARED: &str = "FixtureCompared";
const EXCLUDED: &str = "SEntityInteractableParams";
const LINKED: &str = "FixtureComparedLinks";
const MISSING_GUID: u128 = 41;

#[test]
fn complete_negative_component_shapes_are_ineligible() -> TestResult {
    let database = fixtures(TestType::String)
        .record(item(1, vec![]))
        .record(item(2, vec![compared(true)]))
        .record(item(3, vec![attach(TestValue::text("")), compared(true)]))
        .record(item(4, vec![attachment(TestValue::Null), compared(true)]))
        .record(item(
            5,
            vec![valid_attach(), TestValue::Null, invalid_flag(EXCLUDED)],
        ));
    check(&database, ComponentKeyRead::Ineligible)
}

#[test]
fn failed_component_reads_are_unavailable() -> TestResult {
    let database = fixtures(TestType::String)
        .record(item(
            1,
            vec![invalid_pointer(), valid_attach(), compared(true)],
        ))
        .record(item(
            2,
            vec![valid_attach(), compared(true), invalid_pointer()],
        ))
        .record(item(3, vec![attachment(invalid_pointer()), compared(true)]))
        .record(item(
            4,
            vec![
                attach(TestValue::Raw((u32::MAX - 1).to_le_bytes().to_vec())),
                compared(true),
            ],
        ))
        .record(item(5, vec![valid_attach(), invalid_flag(COMPARED)]));
    check(&database, ComponentKeyRead::Unavailable)
}

#[test]
fn unreadable_attachment_payload_cannot_create_a_compared_member() -> TestResult {
    let incomplete_definition = || attachment(invalid_pointer());
    let incomplete_type = || attach(TestValue::Raw((u32::MAX - 1).to_le_bytes().to_vec()));
    let only_attachment = fixtures(TestType::String)
        .record(item(1, vec![incomplete_definition(), TestValue::Null]))
        .record(item(2, vec![incomplete_type(), invalid_flag(EXCLUDED)]))
        .record(item(3, vec![invalid_attachment_field()]));
    check(&only_attachment, ComponentKeyRead::Ineligible)?;

    let unknown_membership = fixtures(TestType::String)
        .record(item(1, vec![incomplete_definition(), invalid_pointer()]))
        .record(item(2, vec![invalid_attachment_field(), invalid_pointer()]));
    check(&unknown_membership, ComponentKeyRead::Unavailable)?;

    let compared_member = fixtures(TestType::String)
        .record(item(1, vec![invalid_attachment_field(), compared(true)]));
    check(&compared_member, ComponentKeyRead::Unavailable)
}

#[test]
fn unsupported_attachment_type_storage_is_unavailable() -> TestResult {
    let database = fixtures(TestType::Bool)
        .record(item(1, vec![attach(TestValue::Bool(true)), compared(true)]));
    check(&database, ComponentKeyRead::Unavailable)
}

#[test]
fn excluded_payload_and_authored_null_do_not_change_a_comparable_key() -> TestResult {
    let bytes = fixtures(TestType::String)
        .record(item(1, vec![valid_attach(), compared(true)]))
        .record(item(
            2,
            vec![
                valid_attach(),
                TestValue::Null,
                invalid_flag(EXCLUDED),
                compared(true),
            ],
        ))
        .build();
    let forge = DataForge::open(&bytes)?;
    let mut records = forge.records();
    let left = records.next().ok_or("original comparable Item")?.tree()?;
    let right = records
        .next()
        .ok_or("original Item with excluded payload")?
        .tree()?;
    let ComponentKeyRead::Comparable(expected) = read_component_key(&left) else {
        return Err("the authored future type is comparable".into());
    };
    assert_eq!(
        read_component_key(&right),
        ComponentKeyRead::Comparable(expected),
        "only the same compared values enter the digest"
    );
    assert_eq!(
        component_key(&right),
        Some(expected),
        "the existing wrapper preserves identity"
    );
    Ok(())
}

#[test]
fn compared_missing_reference_proves_exclusion_with_its_original_guid_and_census() -> TestResult {
    let bytes = linked_fixture()
        .record(item(1, vec![valid_attach(), linked(MISSING_GUID)]))
        .build();
    test_support::with_input(&bytes, |forge, input| {
        let record = forge.record(0).ok_or("physical Item Record")?;
        let tree = record.tree()?;
        let status = read_component_key(&tree);
        assert_eq!(
            status,
            ComponentKeyRead::UnresolvedLinks,
            "an exhausted authored GUID lookup excludes the Item from comparison"
        );
        assert_eq!(
            component_key(&tree),
            None,
            "legacy comparison still excludes the known missing target"
        );
        let scope = input.record_scope(&record, &tree)?;
        let originals = OriginalFlowFields::read(&scope, None)?;
        let link = originals
            .facts()
            .iter()
            .find(|fact| matches!(fact.value(), Value::Error(_)))
            .ok_or("original array Reference fact")?;
        let expected = "00000000-0000-0000-0000-000000000029".parse()?;
        let guid = link
            .dangling_guid(SourceUseRole::Selector)?
            .ok_or("explicit failed GUID lookup")?;
        assert_eq!(
            guid.value(),
            &SourceValue::Present(expected),
            "the original authored nonzero GUID is retained"
        );
        let proof = capture_component_key(&tree, &originals, status)?;
        assert!(
            proof.origin.source_set_complete(),
            "the exact exclusion is fully proved"
        );
        let table = forge
            .record_table_span()
            .ok_or("validated Record table span")?;
        let count = DataForgeHeaderField::RecordCount.byte_span();
        let inventory = input.origins().uses.freeze()?;
        let guid = link.location(OriginalValuePart::Error(OriginalErrorParameter::Guid));
        for expected in [
            guid,
            span_location("dataforgeRecordTable", "guid", table),
            span_location("dataforgeHeader", "recordCount", count),
        ] {
            assert!(
                inventory.iter_uses().any(|used| {
                    used.location() == &expected
                        && used.role() == SourceUseRole::Selector
                        && proof.origin.source_uses_refs().contains(used.id())
                }),
                "the proved exclusion includes the original operand at {expected:?}"
            );
        }
        Ok(())
    })
}

#[test]
fn an_unreadable_compared_value_still_makes_the_key_unavailable() -> TestResult {
    let database = linked_fixture()
        .record(item(
            1,
            vec![valid_attach(), linked(MISSING_GUID), invalid_flag(COMPARED)],
        ))
        .record(item(
            2,
            vec![valid_attach(), invalid_flag(COMPARED), linked(MISSING_GUID)],
        ));
    check(&database, ComponentKeyRead::Unavailable)
}

#[test]
fn an_excluded_missing_reference_does_not_enter_the_comparison() -> TestResult {
    let inventory = |value| {
        TestValue::Struct(
            TestInstance::new(super::INVENTORY).value("openInventoryInteraction", value),
        )
    };
    let bytes = linked_fixture()
        .structure(
            TestStruct::new(super::INVENTORY)
                .parent(COMPONENT)
                .field("openInventoryInteraction", TestType::Reference),
        )
        .record(item(1, vec![valid_attach(), inventory(TestValue::Null)]))
        .record(item(
            2,
            vec![
                valid_attach(),
                inventory(TestValue::Reference(MISSING_GUID)),
            ],
        ))
        .build();
    let forge = DataForge::open(&bytes)?;
    let baseline = forge.record(0).ok_or("null control")?.tree()?;
    let omitted = forge
        .record(1)
        .ok_or("excluded Reference control")?
        .tree()?;
    let baseline = component_key(&baseline).ok_or("comparable null control")?;
    assert_eq!(
        component_key(&omitted),
        Some(baseline),
        "a missing target in a skipped field does not affect identical keys"
    );
    Ok(())
}

fn span_location(
    namespace: &str,
    field: &str,
    span: std::ops::Range<u64>,
) -> p4kx_extraction::SourceLocation {
    p4kx_extraction::SourceLocation::Subresource {
        namespace: namespace.to_owned(),
        indices: vec![span.start, span.end],
        field: field.to_owned(),
    }
}

fn linked_fixture() -> DataForgeBuilder {
    fixtures(TestType::String).structure(
        TestStruct::new(LINKED)
            .parent(COMPONENT)
            .array("links", TestType::Reference),
    )
}

fn linked(guid: u128) -> TestValue {
    let links = TestValue::Array(vec![TestValue::Null, TestValue::Reference(guid)]);
    TestValue::Struct(TestInstance::new(LINKED).value("links", links))
}

fn check(database: &DataForgeBuilder, expected: ComponentKeyRead<'_>) -> TestResult {
    let bytes = database.build();
    let forge = DataForge::open(&bytes)?;
    for record in forge.records() {
        let tree = record.tree()?;
        assert_eq!(
            read_component_key(&tree),
            expected,
            "original Record {}",
            record.index()
        );
        assert_eq!(
            component_key(&tree),
            None,
            "existing wrapper returns no comparable key"
        );
    }
    Ok(())
}

fn fixtures(item_type: TestType) -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new(ITEM)
                .array("Components", TestType::StrongPointer(COMPONENT.to_owned())),
        )
        .structure(TestStruct::new(COMPONENT))
        .structure(
            TestStruct::new(ATTACHABLE)
                .parent(COMPONENT)
                .field("AttachDef", TestType::StrongPointer(DEFINITION.to_owned())),
        )
        .structure(
            TestStruct::new(DEFINITION)
                .field("Type", item_type)
                .field("additional", TestType::Bool),
        )
        .structure(
            TestStruct::new(COMPARED)
                .parent(COMPONENT)
                .field("flag", TestType::Bool),
        )
        .structure(
            TestStruct::new(EXCLUDED)
                .parent(COMPONENT)
                .field("flag", TestType::Bool),
        )
}

fn item(guid: u128, components: Vec<TestValue>) -> TestRecord {
    TestRecord::new(
        guid,
        ITEM,
        format!("Fixture.Item{guid}"),
        format!("fixture/item{guid}.xml"),
    )
    .value("Components", TestValue::Array(components))
}

fn valid_attach() -> TestValue {
    attach(TestValue::text("FixtureFutureType"))
}

fn attach(kind: TestValue) -> TestValue {
    attachment(TestValue::Struct(
        TestInstance::new(DEFINITION).value("Type", kind),
    ))
}

fn attachment(definition: TestValue) -> TestValue {
    TestValue::Struct(TestInstance::new(ATTACHABLE).value("AttachDef", definition))
}

fn compared(flag: bool) -> TestValue {
    TestValue::Struct(TestInstance::new(COMPARED).value("flag", TestValue::Bool(flag)))
}

fn invalid_attachment_field() -> TestValue {
    attachment(TestValue::Struct(
        TestInstance::new(DEFINITION)
            .value("Type", TestValue::text("FixtureFutureType"))
            .value("additional", TestValue::Raw(vec![2])),
    ))
}

fn invalid_flag(component: &str) -> TestValue {
    TestValue::Struct(TestInstance::new(component).value("flag", TestValue::Raw(vec![2])))
}

fn invalid_pointer() -> TestValue {
    TestValue::Raw([(u32::MAX - 1).to_le_bytes(), 2_u32.to_le_bytes()].concat())
}
