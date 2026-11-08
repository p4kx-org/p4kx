// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestValue};

use crate::commodity_v3_fixture::{
    ALPHA, BETA, DANGLING_RESOURCE, GROUP_FIRST, GROUP_SECOND, QUANTIZATION, RESOURCE,
    RESOURCE_FALLBACK, RESOURCE_UNUSED, UNLINKED,
};
use crate::commodity_v3_fixture_schema;

pub(crate) fn database() -> DataForgeBuilder {
    with_keys("@own_untranslated", "@own_description", "@resource_name")
}

pub(crate) fn with_keys(
    own_name: &str,
    own_description: &str,
    resource_name: &str,
) -> DataForgeBuilder {
    commodity_v3_fixture_schema::database()
        .record(primary_resource(resource_name))
        .record(
            resource(RESOURCE_FALLBACK, "Fallback", "@fallback_resource_name").value(
                "properties",
                TestValue::Array(vec![owned(
                    "ResourceTypeCraftingData",
                    [
                        ("name", TestValue::text("Quantized")),
                        (
                            "qualityQuantization",
                            owned(
                                "CraftingQualityQuantization_RecordRef",
                                [(
                                    "qualityQuantizationRecord",
                                    TestValue::Reference(QUANTIZATION),
                                )],
                            ),
                        ),
                    ],
                )]),
            ),
        )
        .record(resource(RESOURCE_UNUSED, "Unused", "@unused_name"))
        .record(group(
            GROUP_FIRST,
            "First",
            vec![RESOURCE, RESOURCE_FALLBACK],
        ))
        .record(group(GROUP_SECOND, "Second", vec![RESOURCE]))
        .record(quantization())
        .record(alpha(own_name, own_description))
        .record(entity(
            BETA,
            "Beta",
            vec![
                purchase("@LOC_PLACEHOLDER", ""),
                container(vec![composition(Some(RESOURCE))]),
            ],
            vec![display("@LOC_PLACEHOLDER")],
        ))
        .record(entity(
            UNLINKED,
            "Unlinked",
            vec![purchase("", "")],
            Vec::new(),
        ))
}

pub(crate) fn dangling_first() -> DataForgeBuilder {
    commodity_v3_fixture_schema::database()
        .record(resource(RESOURCE, "Readable", "@resource_name"))
        .record(entity(
            ALPHA,
            "DanglingFirst",
            vec![
                purchase("@own_untranslated", ""),
                container(vec![
                    composition(Some(DANGLING_RESOURCE)),
                    composition(Some(RESOURCE)),
                ]),
            ],
            Vec::new(),
        ))
}

fn alpha(name: &str, description: &str) -> TestRecord {
    entity(
        ALPHA,
        "Alpha",
        vec![
            TestValue::Null,
            owned("FixtureUnrelated", []),
            purchase(name, "@own_type"),
            container(vec![
                TestValue::Null,
                composition(None),
                composition(Some(RESOURCE)),
                composition(Some(RESOURCE_FALLBACK)),
            ]),
            purchase("@later_name", "@later_type"),
            container(vec![composition(Some(RESOURCE_FALLBACK))]),
        ],
        vec![
            TestValue::Null,
            display(description),
            display("@later_description"),
        ],
    )
}

fn resource(guid: u128, name: &str, key: &str) -> TestRecord {
    TestRecord::new(
        guid,
        "ResourceType",
        format!("ResourceType.{name}"),
        format!("libs/foundry/records/resourcetypedatabase/{name}.xml"),
    )
    .value("displayName", TestValue::text(key))
    .value("description", TestValue::text("@resource_description"))
}

fn group(guid: u128, name: &str, resources: Vec<u128>) -> TestRecord {
    TestRecord::new(
        guid,
        "ResourceTypeGroup",
        format!("ResourceTypeGroup.{name}"),
        format!("libs/foundry/records/resourcetypedatabase/group_{name}.xml"),
    )
    .value(
        "resources",
        TestValue::Array(resources.into_iter().map(TestValue::Reference).collect()),
    )
}

fn quantization() -> TestRecord {
    TestRecord::new(
        QUANTIZATION,
        "CraftingQualityQuantizationRecord",
        "CraftingQualityQuantizationRecord.Fixture",
        "libs/foundry/records/crafting/qualityquantization/fixture.xml",
    )
    .value(
        "qualityQuantization",
        owned(
            "CraftingQualityQuantization",
            [(
                "bands",
                TestValue::Array(vec![owned(
                    "CraftingQualityQuantizationBand",
                    [
                        ("start", TestValue::Int32(0)),
                        ("end", TestValue::Int32(100)),
                        ("mappedValue", TestValue::Int32(50)),
                    ],
                )]),
            )],
        ),
    )
}

fn entity(
    guid: u128,
    name: &str,
    components: Vec<TestValue>,
    display: Vec<TestValue>,
) -> TestRecord {
    TestRecord::new(
        guid,
        "EntityClassDefinition",
        format!("EntityClassDefinition.{name}"),
        format!("libs/foundry/records/entities/commodities/{name}.xml"),
    )
    .value("Components", TestValue::Array(components))
    .value("StaticEntityClassData", TestValue::Array(display))
}

fn purchase(name: &str, display_type: &str) -> TestValue {
    owned(
        "SCItemPurchasableParams",
        [
            ("displayName", TestValue::text(name)),
            ("displayType", TestValue::text(display_type)),
        ],
    )
}

fn display(description: &str) -> TestValue {
    owned(
        "EntityUIDisplayParams",
        [("displayDescription", TestValue::text(description))],
    )
}

fn container(composition: Vec<TestValue>) -> TestValue {
    owned(
        "ResourceContainer",
        [("defaultComposition", TestValue::Array(composition))],
    )
}

fn composition(resource: Option<u128>) -> TestValue {
    owned(
        "FixtureComposition",
        [(
            "entry",
            resource.map_or(TestValue::Null, TestValue::Reference),
        )],
    )
}

fn owned(
    type_name: &str,
    fields: impl IntoIterator<Item = (&'static str, TestValue)>,
) -> TestValue {
    let mut value = TestInstance::new(type_name);
    for (name, field) in fields {
        value = value.value(name, field);
    }
    TestValue::Struct(value)
}

fn primary_resource(name: &str) -> TestRecord {
    resource(RESOURCE, "Alpha", name)
        .value("refinedVersion", TestValue::Reference(RESOURCE_FALLBACK))
        .value(
            "densityType",
            owned(
                "ResourceTypeDensity",
                [(
                    "densityUnit",
                    owned(
                        "GramsPerCubicCentimeter",
                        [("gramsPerCubicCentimeter", TestValue::Single(3.25))],
                    ),
                )],
            ),
        )
}
