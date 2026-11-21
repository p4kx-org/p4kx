// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

pub(crate) const RESOURCE: u128 = 0xa101;
pub(crate) const COMMODITY: u128 = 0xa201;
pub(crate) const FIRST_GROUP: u128 = 0xa301;
pub(crate) const SECOND_GROUP: u128 = 0xa302;
pub(crate) const CHILD_GROUP: u128 = 0xa303;
pub(crate) const MISSING: u128 = 0xafff;

#[derive(Clone, Copy)]
pub(crate) enum LinkStorage {
    TypedReference,
    WeakRecordPointer,
}

fn schema(storage: LinkStorage) -> DataForgeBuilder {
    schema_with_resource(
        storage,
        TestStruct::new("ResourceType").field("displayName", TestType::Locale),
    )
}

fn schema_with_resource(storage: LinkStorage, resource: TestStruct) -> DataForgeBuilder {
    let entry = match storage {
        LinkStorage::TypedReference => TestType::ReferenceTo("ResourceType".to_owned()),
        LinkStorage::WeakRecordPointer => TestType::WeakPointer("ResourceType".to_owned()),
    };
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("FixtureComponent"))
        .structure(TestStruct::new("EntityClassDefinition").array(
            "Components",
            TestType::StrongPointer("FixtureComponent".to_owned()),
        ))
        .structure(
            TestStruct::new("ResourceContainer")
                .parent("FixtureComponent")
                .array(
                    "defaultComposition",
                    TestType::Class("FixtureCompositionEntry".to_owned()),
                ),
        )
        .structure(TestStruct::new("FixtureCompositionEntry").field("entry", entry))
        .structure(resource)
        .structure(
            TestStruct::new("ResourceTypeGroup")
                .array(
                    "groups",
                    TestType::ReferenceTo("ResourceTypeGroup".to_owned()),
                )
                .array(
                    "resources",
                    TestType::ReferenceTo("ResourceType".to_owned()),
                ),
        )
}

fn resource(name: &str, scope: &str, key: &str) -> TestRecord {
    TestRecord::new(
        RESOURCE,
        "ResourceType",
        format!("ResourceType.{name}"),
        format!("libs/foundry/records/resourcetypedatabase/{scope}/{name}.xml"),
    )
    .value("displayName", TestValue::text(key))
}

fn commodity(entries: Vec<TestValue>) -> TestRecord {
    let entries = entries
        .into_iter()
        .map(|entry| {
            TestValue::Struct(TestInstance::new("FixtureCompositionEntry").value("entry", entry))
        })
        .collect();
    TestRecord::new(
        COMMODITY,
        "EntityClassDefinition",
        "EntityClassDefinition.FixtureCommodity",
        "libs/foundry/records/entities/commodities/included/fixture_commodity.xml",
    )
    .value(
        "Components",
        TestValue::Array(vec![TestValue::Struct(
            TestInstance::new("ResourceContainer")
                .value("defaultComposition", TestValue::Array(entries)),
        )]),
    )
}

pub(crate) fn dangling_first() -> DataForgeBuilder {
    schema(LinkStorage::TypedReference)
        .record(resource("FixtureResource", "included", "@fixture_resource"))
        .record(commodity(vec![
            TestValue::Reference(MISSING),
            TestValue::Reference(RESOURCE),
        ]))
}

pub(crate) fn null_first() -> DataForgeBuilder {
    schema(LinkStorage::TypedReference)
        .record(resource("FixtureResource", "included", "@fixture_resource"))
        .record(commodity(vec![
            TestValue::Null,
            TestValue::Reference(RESOURCE),
        ]))
}

pub(crate) fn aliases(storage: LinkStorage) -> DataForgeBuilder {
    let entry = match storage {
        LinkStorage::TypedReference => TestValue::Reference(RESOURCE),
        LinkStorage::WeakRecordPointer => TestValue::pointer("ResourceType.FixtureEmitted"),
    };
    schema(storage)
        .record(resource("FixtureOriginal", "excluded", "@fixture_original"))
        .record(resource("FixtureEmitted", "included", "@fixture_emitted"))
        .record(commodity(vec![entry]))
}

pub(crate) fn unreadable_resource() -> DataForgeBuilder {
    let definition = TestStruct::new("ResourceType")
        .field("displayName", TestType::Locale)
        .declared_size(5);
    schema_with_resource(LinkStorage::TypedReference, definition)
        .record(resource(
            "FixtureUnreadable",
            "included",
            "@fixture_resource",
        ))
        .record(commodity(vec![TestValue::Reference(RESOURCE)]))
}

fn group(guid: u128, name: &str, scope: &str, resources: &[u128], children: &[u128]) -> TestRecord {
    TestRecord::new(
        guid,
        "ResourceTypeGroup",
        format!("ResourceTypeGroup.{name}"),
        format!("libs/foundry/records/resourcetypedatabase/{scope}/{name}.xml"),
    )
    .value(
        "resources",
        TestValue::Array(
            resources
                .iter()
                .map(|guid| TestValue::Reference(*guid))
                .collect(),
        ),
    )
    .value(
        "groups",
        TestValue::Array(
            children
                .iter()
                .map(|guid| TestValue::Reference(*guid))
                .collect(),
        ),
    )
}

pub(crate) fn group_filter() -> DataForgeBuilder {
    schema(LinkStorage::TypedReference)
        .record(resource("FixtureResource", "included", "@fixture_resource"))
        .record(commodity(vec![TestValue::Reference(RESOURCE)]))
        .record(group(
            FIRST_GROUP,
            "FixtureEarlier",
            "excluded",
            &[RESOURCE],
            &[CHILD_GROUP],
        ))
        .record(group(
            SECOND_GROUP,
            "FixtureSelected",
            "included",
            &[RESOURCE],
            &[CHILD_GROUP],
        ))
        .record(group(CHILD_GROUP, "FixtureChild", "included", &[], &[]))
}

pub(crate) fn reference_budget() -> DataForgeBuilder {
    let resources = vec![RESOURCE; 6000];
    schema(LinkStorage::TypedReference)
        .record(resource("FixtureResource", "included", "@fixture_resource"))
        .record(group(
            FIRST_GROUP,
            "FixtureManyReferences",
            "included",
            &resources,
            &[],
        ))
}

pub(crate) fn unreadable_type_metadata() -> DataForgeBuilder {
    schema(LinkStorage::TypedReference)
        .record(resource("FixtureResource", "included", "@fixture_resource"))
        .record(TestRecord::new(
            MISSING,
            "FixtureUndeclaredType",
            "FixtureUndeclaredType.Metadata",
            "libs/foundry/records/resourcetypedatabase/excluded/metadata.xml",
        ))
        .record(commodity(vec![TestValue::Reference(RESOURCE)]))
}

pub(crate) fn unreadable_type_name() -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let name = "FixtureUnreadableRecordType";
    let mut bytes = schema(LinkStorage::TypedReference)
        .structure(TestStruct::new(name))
        .record(resource("FixtureResource", "included", "@fixture_resource"))
        .record(TestRecord::new(
            MISSING,
            name,
            "FixtureMetadata",
            "libs/foundry/records/resourcetypedatabase/excluded/metadata.xml",
        ))
        .record(commodity(vec![TestValue::Reference(RESOURCE)]))
        .build();
    let position = {
        let mut matches = bytes
            .windows(name.len())
            .enumerate()
            .filter_map(|(index, value)| (value == name.as_bytes()).then_some(index));
        let position = matches.next().ok_or("the synthetic struct-name token")?;
        if matches.next().is_some() {
            return Err("the synthetic struct-name token must be unique".into());
        }
        position
    };
    *bytes
        .get_mut(position)
        .ok_or("the synthetic struct-name byte")? = 0xff;
    Ok(bytes)
}
