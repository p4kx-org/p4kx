// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

pub(super) const PRIMARY: u128 = 0x8130;
pub(super) const SECONDARY: u128 = 0x8131;
pub(super) const OWNER: u128 = 0x8132;
pub(super) const MISSING: u128 = 0x8133;
const COMPONENT: &str = "SAmmoContainerComponentParams";
const FRAGMENT: &str = "SFragmentParams";
const HIDE: &str = "SHideAmmoAttachmentParams";
const ATTACHABLE: &str = "SAttachableEntityParams";
const CONTAINER_ENUM: &str = "EAmmoContainerType";

#[derive(Clone, Copy)]
pub(super) enum Case {
    Valid,
    NoSecondaryDeclaration,
    BadLinks,
    WrongDeclaration,
}

pub(super) fn database(case: Case) -> DataForgeBuilder {
    let builder = DataForgeBuilder::new()
        .version(8)
        .enumeration(CONTAINER_ENUM, ["FixtureMagazine"])
        .structure(TestStruct::new("AmmoParams").field("speed", TestType::Single))
        .structure(TestStruct::new("EntityClassDefinition").array(
            "Components",
            TestType::StrongPointer("DataForgeComponentParams".to_owned()),
        ))
        .structure(TestStruct::new("DataForgeComponentParams"))
        .structure(TestStruct::new("FixtureOtherComponent").parent("DataForgeComponentParams"))
        .structure(container_type(case));
    attachment_types(builder)
        .record(ammo(PRIMARY, "fixture_primary"))
        .record(ammo(SECONDARY, "fixture_secondary"))
        .record(owner(case))
}

fn container_type(case: Case) -> TestStruct {
    let target = match case {
        Case::WrongDeclaration => "EntityClassDefinition",
        _ => "AmmoParams",
    };
    let base = TestStruct::new(COMPONENT)
        .parent("DataForgeComponentParams")
        .field("initialAmmoCount", TestType::Int32)
        .field("maxAmmoCount", TestType::Int32)
        .field("maxRestockCount", TestType::UInt32)
        .field("lowAmmoWarningPercentage", TestType::Single)
        .field("ammoParamsRecord", TestType::ReferenceTo(target.to_owned()))
        .field(
            "ammoContainerType",
            TestType::Enum(CONTAINER_ENUM.to_owned()),
        )
        .field("despawnEmptyAmmoContainer", TestType::Bool)
        .field("allowAmmoRepool", TestType::Bool)
        .field("emptyGeometryTag", TestType::String)
        .field("ammoCountFragment", TestType::Class(FRAGMENT.to_owned()))
        .field("ammoCountAnimationBlendTime", TestType::Single)
        .array("hideAttachments", TestType::Class(HIDE.to_owned()))
        .array("attachableEntities", TestType::Class(ATTACHABLE.to_owned()));
    match case {
        Case::NoSecondaryDeclaration => base,
        Case::Valid | Case::BadLinks | Case::WrongDeclaration => base.field(
            "secondaryAmmoParamsRecord",
            TestType::ReferenceTo(target.to_owned()),
        ),
    }
}

fn attachment_types(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder
        .structure(
            TestStruct::new(FRAGMENT)
                .field("fragment", TestType::String)
                .field("forceWeaponController", TestType::Bool),
        )
        .structure(
            TestStruct::new(HIDE)
                .field("attachmentName", TestType::String)
                .field("ammoThreshold", TestType::Int32)
                .field("visible", TestType::Bool),
        )
        .structure(
            TestStruct::new(ATTACHABLE)
                .field("sourcePort", TestType::String)
                .field("targetPort", TestType::String)
                .field("entityClassName", TestType::String),
        )
}

fn owner(case: Case) -> TestRecord {
    let components = vec![
        TestValue::Struct(TestInstance::new("FixtureOtherComponent")),
        TestValue::Struct(first_container(case)),
        TestValue::Struct(second_container()),
    ];
    TestRecord::new(
        OWNER,
        "EntityClassDefinition",
        "EntityClassDefinition.FixtureContainer",
        "libs/foundry/records/entities/scitem/fixture_container.xml",
    )
    .value("Components", TestValue::Array(components))
}

fn first_container(case: Case) -> TestInstance {
    let container = container(-7)
        .value(
            "hideAttachments",
            TestValue::Array(vec![
                hidden("fixture-before", 7, false),
                hidden("fixture-after", 0, true),
            ]),
        )
        .value("attachableEntities", TestValue::Array(Vec::new()));
    match case {
        Case::Valid => container
            .value("ammoParamsRecord", TestValue::Reference(PRIMARY))
            .value("secondaryAmmoParamsRecord", TestValue::Reference(SECONDARY)),
        Case::NoSecondaryDeclaration => {
            container.value("ammoParamsRecord", TestValue::Reference(PRIMARY))
        }
        Case::WrongDeclaration => container
            .value("ammoParamsRecord", TestValue::Reference(OWNER))
            .value("secondaryAmmoParamsRecord", TestValue::Reference(OWNER)),
        Case::BadLinks => container
            .value("ammoParamsRecord", TestValue::Reference(MISSING))
            .value("secondaryAmmoParamsRecord", TestValue::Reference(OWNER)),
    }
}

fn second_container() -> TestInstance {
    let attachment = TestInstance::new(ATTACHABLE)
        .value("sourcePort", TestValue::text("fixture-source"))
        .value("targetPort", TestValue::text("fixture-target"))
        .value("entityClassName", TestValue::text("fixture-class"));
    container(0)
        .value("ammoParamsRecord", TestValue::Reference(SECONDARY))
        .value("hideAttachments", TestValue::Array(Vec::new()))
        .value(
            "attachableEntities",
            TestValue::Array(vec![TestValue::Struct(attachment)]),
        )
}

fn container(capacity: i32) -> TestInstance {
    let fragment = TestInstance::new(FRAGMENT)
        .value("fragment", TestValue::text("fixture-fragment"))
        .value("forceWeaponController", TestValue::Bool(false));
    TestInstance::new(COMPONENT)
        .value("initialAmmoCount", TestValue::Int32(0))
        .value("maxAmmoCount", TestValue::Int32(capacity))
        .value("maxRestockCount", TestValue::UInt32(u32::MAX))
        .value("lowAmmoWarningPercentage", TestValue::Single(-0.0))
        .value("ammoContainerType", TestValue::text("FixtureMagazine"))
        .value("despawnEmptyAmmoContainer", TestValue::Bool(false))
        .value("allowAmmoRepool", TestValue::Bool(true))
        .value("emptyGeometryTag", TestValue::text("fixture-empty"))
        .value("ammoCountFragment", TestValue::Struct(fragment))
        .value("ammoCountAnimationBlendTime", TestValue::Single(1.0))
}

fn hidden(name: &str, threshold: i32, visible: bool) -> TestValue {
    TestValue::Struct(
        TestInstance::new(HIDE)
            .value("attachmentName", TestValue::text(name))
            .value("ammoThreshold", TestValue::Int32(threshold))
            .value("visible", TestValue::Bool(visible)),
    )
}

fn ammo(guid: u128, name: &str) -> TestRecord {
    TestRecord::new(
        guid,
        "AmmoParams",
        format!("AmmoParams.{name}"),
        format!("libs/foundry/records/ammoparams/fixture/{name}.xml"),
    )
    .value("speed", TestValue::Single(8.0))
}
