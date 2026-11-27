// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{TestInstance, TestRecord, TestValue};

use super::{ENTITY, HARVESTABLE, PRESETS, RESOURCE, record};

fn harvestable_entry(harvestable: Option<u128>, entity: Option<u128>) -> TestInstance {
    let mut element = TestInstance::new("HarvestableElement")
        .value("relativeProbability", TestValue::Single(22.0))
        .value("clustering", TestValue::Reference(RESOURCE))
        .value(
            "geometries",
            TestValue::Array(vec![TestValue::Struct(
                TestInstance::new("HarvestableGeometry")
                    .value("tag", TestValue::Reference(RESOURCE)),
            )]),
        );
    if let Some(harvestable) = harvestable {
        element = element.value("harvestable", TestValue::Reference(harvestable));
    }
    if let Some(entity) = entity {
        element = element.value("harvestableEntityClass", TestValue::Reference(entity));
    }
    element
}

pub(super) fn provider(guid: u128, name: &str, with_entity_only: bool) -> TestRecord {
    let modifier_target = format!("{name}_provider_element");
    let preset = harvestable_entry(Some(HARVESTABLE), None).named(&modifier_target);
    let mut elements = vec![TestValue::Struct(preset)];
    if with_entity_only {
        elements.push(TestValue::Struct(harvestable_entry(None, Some(ENTITY))));
    }
    record(
        guid,
        "HarvestableProviderPreset",
        name,
        &format!("{PRESETS}{}", name.to_lowercase()),
    )
    .value(
        "harvestableGroups",
        TestValue::Array(vec![TestValue::Struct(
            TestInstance::new("HarvestableElementGroup")
                .value("groupName", TestValue::text("SpaceShip_Mineables"))
                .value("groupProbability", TestValue::Single(6.0))
                .value("harvestables", TestValue::Array(elements)),
        )]),
    )
    .value(
        "areas",
        TestValue::Array(vec![TestValue::Struct(
            TestInstance::new("HarvestableAreaPreset")
                .value("debugGroupName", TestValue::text("Acidic"))
                .value("globalModifier", TestValue::Single(1.0))
                .value(
                    "modifiers",
                    TestValue::Array(vec![TestValue::Struct(
                        TestInstance::new("HarvestableAreaModifier")
                            .value("harvestableElement", TestValue::pointer(&modifier_target))
                            .value("harvestableModifier", TestValue::Single(0.0)),
                    )]),
                )
                .value(
                    "areaType",
                    TestValue::Struct(TestInstance::new("HarvestableAreaTypeObjectPreset").value(
                        "objectPresetPaths",
                        TestValue::Array(vec![TestValue::text(
                            "Libs/ObjectPresets/PU/system/stanton/hurston/acidic_pools_01.opr",
                        )]),
                    )),
                ),
        )]),
    )
}
