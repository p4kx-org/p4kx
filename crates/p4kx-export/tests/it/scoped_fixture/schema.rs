// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestStruct, TestType};

fn pointer(name: &str) -> TestType {
    TestType::StrongPointer(name.to_owned())
}

fn class(name: &str) -> TestType {
    TestType::Class(name.to_owned())
}

pub(super) fn add(builder: DataForgeBuilder) -> DataForgeBuilder {
    [
        owners(),
        mission_properties(),
        relations(),
        definitions(),
        unknowns(),
        contracts(),
    ]
    .into_iter()
    .flatten()
    .fold(
        builder
            .enumeration("ReactionType", ["Hostile", "FutureHostility"])
            .enumeration("ContractScope", ["Local", "FutureScope"]),
        DataForgeBuilder::structure,
    )
}

fn owners() -> Vec<TestStruct> {
    vec![
        TestStruct::new("ContractTemplate")
            .field("wrongTarget", pointer("OtherTarget"))
            .array("contractProperties", class("MissionProperty"))
            .array("objectiveTokens", class("ObjectiveToken")),
        TestStruct::new("MissionBrokerEntry").array("properties", class("MissionProperty")),
        TestStruct::new("OtherTarget").field("missionVariableName", TestType::String),
        TestStruct::new("ObjectiveToken")
            .field("id", TestType::Guid)
            .array("properties", pointer("ObjectiveProperty")),
        TestStruct::new("ObjectiveProperty"),
        TestStruct::new("ObjectiveProperty_Embedded")
            .parent("ObjectiveProperty")
            .field("property", class("MissionProperty")),
    ]
}

fn mission_properties() -> Vec<TestStruct> {
    vec![
        TestStruct::new("MissionProperty")
            .field("missionVariableName", TestType::String)
            .field("extendedTextToken", TestType::String)
            .field("value", pointer("BaseMissionPropertyValue")),
        TestStruct::new("BaseMissionPropertyValue"),
        TestStruct::new("MissionPropertyValue_Object").parent("BaseMissionPropertyValue"),
        TestStruct::new("MissionPropertyValue_Boolean")
            .parent("BaseMissionPropertyValue")
            .field("value", TestType::Bool),
        TestStruct::new("MissionPropertyValue_Unknown").parent("BaseMissionPropertyValue"),
        TestStruct::new("MissionPropertyValue_ScopedLocation")
            .parent("BaseMissionPropertyValue")
            .field("factionFilter", pointer("LocationCondition_FactionBase"))
            .field(
                "locationClassFilter",
                pointer("LocationCondition_LocationClassBase"),
            )
            .field(
                "locationSizeFilter",
                pointer("LocationCondition_LocationSizeBase"),
            )
            .field(
                "locationFunctionFilter",
                pointer("LocationCondition_LocationFunctionBase"),
            )
            .field(
                "otherLocationFilter",
                pointer("LocationCondition_OtherLocationBase"),
            )
            .field(
                "relativeLocationFilter",
                pointer("LocationCondition_RelativeLocationBase"),
            ),
    ]
}

fn relations() -> Vec<TestStruct> {
    let mut result = vec![
        TestStruct::new("LocationCondition_FactionBase"),
        TestStruct::new("LocationCondition_OtherLocationBase"),
        TestStruct::new("LocationCondition_RelativeLocationBase"),
    ];
    for name in ["RequiredFactionHostility", "ExcludedFactionHostility"] {
        result.push(
            TestStruct::new(format!("LocationCondition_{name}"))
                .parent("LocationCondition_FactionBase")
                .field("hostility", TestType::Enum("ReactionType".into())),
        );
    }
    for name in ["NotLocation", "SameLocation"] {
        result.push(
            TestStruct::new(format!("LocationCondition_{name}"))
                .parent("LocationCondition_OtherLocationBase")
                .field("location", TestType::WeakPointer("MissionProperty".into())),
        );
    }
    result.push(
        TestStruct::new("LocationCondition_NotLocations")
            .parent("LocationCondition_OtherLocationBase")
            .array("locations", TestType::WeakPointer("MissionProperty".into())),
    );
    for name in ["SameScope", "DifferentScope"] {
        result.push(
            TestStruct::new(format!("LocationCondition_{name}"))
                .parent("LocationCondition_RelativeLocationBase")
                .field("scope", TestType::Enum("ContractScope".into()))
                .field("location", TestType::WeakPointer("MissionProperty".into())),
        );
    }
    result
}

fn definitions() -> Vec<TestStruct> {
    let mut result = Vec::new();
    for (suffix, field, plural, target) in [
        (
            "LocationClass",
            "locationClass",
            "locationClasses",
            "MissionLocationClass",
        ),
        (
            "LocationSize",
            "locationSize",
            "locationSizes",
            "MissionLocationSize",
        ),
        (
            "LocationFunction",
            "locationFunction",
            "locationFunctions",
            "MissionLocationFunction",
        ),
    ] {
        let base = format!("LocationCondition_{suffix}Base");
        result.push(TestStruct::new(&base));
        result.push(TestStruct::new(target));
        for polarity in ["Required", "Excluded"] {
            result.push(
                TestStruct::new(format!("LocationCondition_{polarity}{suffix}"))
                    .parent(&base)
                    .field(field, TestType::ReferenceTo(target.to_owned())),
            );
            let ending = if suffix == "LocationClass" { "es" } else { "s" };
            result.push(
                TestStruct::new(format!("LocationCondition_{polarity}{suffix}{ending}"))
                    .parent(&base)
                    .array(plural, TestType::ReferenceTo(target.to_owned())),
            );
        }
    }
    result
}

fn unknowns() -> Vec<TestStruct> {
    [
        ("Faction", "LocationCondition_FactionBase"),
        ("Class", "LocationCondition_LocationClassBase"),
        ("Size", "LocationCondition_LocationSizeBase"),
        ("Function", "LocationCondition_LocationFunctionBase"),
        ("Other", "LocationCondition_OtherLocationBase"),
        ("Relative", "LocationCondition_RelativeLocationBase"),
    ]
    .into_iter()
    .map(|(suffix, base)| {
        TestStruct::new(format!("LocationCondition_Unknown{suffix}")).parent(base)
    })
    .collect()
}

fn contracts() -> Vec<TestStruct> {
    vec![
        TestStruct::new("ContractGenerator")
            .array("generators", pointer("ContractGeneratorHandler")),
        TestStruct::new("ContractGeneratorHandler"),
        TestStruct::new("ContractGeneratorHandler_Career")
            .parent("ContractGeneratorHandler")
            .field("debugName", TestType::String)
            .field("contractParams", pointer("ContractParamOverrides"))
            .array("contracts", class("CareerContract")),
        TestStruct::new("ContractParamOverrides")
            .array("propertyOverrides", pointer("MissionProperty")),
        TestStruct::new("CareerContract")
            .field("id", TestType::Guid)
            .field("debugName", TestType::String)
            .field("template", TestType::Reference)
            .field("missionBrokerEntry", TestType::Reference)
            .field("paramOverrides", pointer("ContractParamOverrides"))
            .array("subContracts", pointer("SubContract")),
        TestStruct::new("SubContract")
            .field("id", TestType::Guid)
            .field("debugName", TestType::String)
            .field("paramOverrides", pointer("ContractParamOverrides")),
        TestStruct::new("DirectSubContract")
            .parent("SubContract")
            .array("propertyOverrides", pointer("MissionProperty")),
    ]
}
