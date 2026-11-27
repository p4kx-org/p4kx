// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

const CRAFTING_ROOT: &str = "libs/foundry/records/crafting";
const BLUEPRINT: &str = "CraftingBlueprintRecord";
const BODY: &str = "FixtureCraftingBlueprint";
const PROCESS: &str = "FixtureCraftingProcess";
const DISMANTLE: &str = "GenericCraftingProcess_Dismantle";
const CREATION: &str = "FixtureCreation";
const TIME: &str = "TimeValue_Partitioned";
const GLOBALS: &str = "CraftingGlobalParams";
const DEFAULTS: &str = "FixtureDefaultBlueprints";
const RESOURCES: &str = "dismantleBlacklistResources";
const DISMANTLE_GUID: u128 = 0x1177_8301;
const CRAFTING_GUID: u128 = 0x1177_8302;
const GLOBAL_GUID: u128 = 0x1177_8303;
const RESOURCE: u128 = 0x1177_8304;
const PRODUCT: u128 = 0x1177_8305;
const FOREIGN: u128 = 0x1177_8306;
const DANGLING: u128 = 0x1177_8307;

pub(super) fn database() -> DataForgeBuilder {
    schema()
        .record(dismantle("FixtureDismantle"))
        .record(
            TestRecord::new(
                CRAFTING_GUID,
                BLUEPRINT,
                "CraftingBlueprintRecord.FixtureCreation",
                format!("{CRAFTING_ROOT}/blueprints/crafting/fixture.xml"),
            )
            .value(
                "blueprint",
                body(
                    TestInstance::new(CREATION).value("entityClass", TestValue::Reference(PRODUCT)),
                ),
            ),
        )
        .record(globals(
            "Primary",
            vec![
                TestValue::Reference(RESOURCE),
                TestValue::Null,
                TestValue::Reference(RESOURCE),
                TestValue::Reference(DANGLING),
                TestValue::Reference(FOREIGN),
            ],
        ))
        .record(globals("Alias", Vec::new()))
        .record(TestRecord::new(
            RESOURCE,
            "ResourceType",
            "FixtureMaterial",
            "libs/foundry/records/resourcetypedatabase/fixture.xml",
        ))
        .record(TestRecord::new(
            PRODUCT,
            "EntityClassDefinition",
            "FixtureProduct",
            "libs/foundry/records/entities/scitem/gear/fixture.xml",
        ))
        .record(TestRecord::new(
            FOREIGN,
            "FixtureForeign",
            "OtherFixture",
            "fixture/other.xml",
        ))
}

pub(super) fn duplicate() -> DataForgeBuilder {
    database().record(dismantle("FixtureDismantleAlias"))
}

fn schema() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new(BLUEPRINT).field("blueprint", pointer(BODY)))
        .structure(TestStruct::new(BODY).field("processSpecificData", pointer(PROCESS)))
        .structure(TestStruct::new(PROCESS))
        .structure(
            TestStruct::new(CREATION)
                .parent(PROCESS)
                .field("entityClass", TestType::Reference),
        )
        .structure(
            TestStruct::new(DISMANTLE)
                .parent(PROCESS)
                .field("efficiency", TestType::Double)
                .field("dismantleTime", TestType::Class(TIME.to_owned())),
        )
        .structure(
            TestStruct::new(TIME)
                .field("days", TestType::Int32)
                .field("hours", TestType::Int32)
                .field("minutes", TestType::Int32)
                .field("seconds", TestType::Single),
        )
        .structure(
            TestStruct::new(GLOBALS)
                .array(RESOURCES, TestType::Reference)
                .array("dismantleBlacklistEntityClasses", TestType::Reference)
                .field("defaultBlueprintSelection", pointer(DEFAULTS)),
        )
        .structure(TestStruct::new(DEFAULTS).array("blueprintRecords", TestType::Reference))
        .structure(TestStruct::new("ResourceType"))
        .structure(TestStruct::new("EntityClassDefinition"))
        .structure(TestStruct::new("FixtureForeign"))
}

fn dismantle(name: &str) -> TestRecord {
    let time = TestInstance::new(TIME)
        .value("days", TestValue::Int32(0))
        .value("hours", TestValue::Int32(0))
        .value("minutes", TestValue::Int32(1))
        .value("seconds", TestValue::Single(14.75));
    let process = TestInstance::new(DISMANTLE)
        .value("efficiency", TestValue::Double(0.625))
        .value("dismantleTime", TestValue::Struct(time));
    TestRecord::new(
        DISMANTLE_GUID,
        BLUEPRINT,
        format!("{BLUEPRINT}.{name}"),
        format!("{CRAFTING_ROOT}/blueprints/dismantle/{name}.xml"),
    )
    .value("blueprint", body(process))
}

fn globals(name: &str, resources: Vec<TestValue>) -> TestRecord {
    TestRecord::new(
        GLOBAL_GUID,
        GLOBALS,
        format!("{GLOBALS}.{name}"),
        format!("{CRAFTING_ROOT}/globalparams/{name}.xml"),
    )
    .value(RESOURCES, TestValue::Array(resources))
    .value(
        "dismantleBlacklistEntityClasses",
        TestValue::Array(Vec::new()),
    )
    .value(
        "defaultBlueprintSelection",
        TestValue::Struct(TestInstance::new(DEFAULTS).value(
            "blueprintRecords",
            TestValue::Array(vec![TestValue::Reference(CRAFTING_GUID)]),
        )),
    )
}

fn body(process: TestInstance) -> TestValue {
    TestValue::Struct(
        TestInstance::new(BODY).value("processSpecificData", TestValue::Struct(process)),
    )
}

fn pointer(target: &str) -> TestType {
    TestType::StrongPointer(target.to_owned())
}
