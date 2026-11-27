// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! A synthetic Archive with contracts that haul, a template and a broker entry with hauling
//! objectives, and cargo manifests.

mod manifests;
mod structures;

use std::error::Error;
use std::path::{Path, PathBuf};

use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, GlobalIniBuilder, TestEntry, TestInstance, TestRecord,
    TestValue,
};

use manifests::{manifest_custom, manifest_even, manifest_random};
use structures::structures;

pub(crate) const GENERATOR: u128 = 0xf000_0000_0000_4000_8000_0000_0000_0001;
pub(crate) const NEAR: u128 = 0xf100_0000_0000_4000_8000_0000_0000_0001;
pub(crate) const FAR: u128 = 0xf100_0000_0000_4000_8000_0000_0000_0002;
pub(crate) const PLAIN: u128 = 0xf100_0000_0000_4000_8000_0000_0000_0003;
pub(crate) const BROKERED: u128 = 0xf100_0000_0000_4000_8000_0000_0000_0004;
pub(crate) const NAMELESS: u128 = 0xf100_0000_0000_4000_8000_0000_0000_0006;
pub(crate) const NOTHING: u128 = 0xf100_0000_0000_4000_8000_0000_0000_0005;
pub(crate) const SUB: u128 = 0xf200_0000_0000_4000_8000_0000_0000_0001;
pub(crate) const TEMPLATE: u128 = 0xf300_0000_0000_4000_8000_0000_0000_0001;
pub(crate) const TEMPLATE_PLAIN: u128 = 0xf300_0000_0000_4000_8000_0000_0000_0002;
pub(crate) const BROKER: u128 = 0xf400_0000_0000_4000_8000_0000_0000_0001;
pub(crate) const SCRAP: u128 = 0xf500_0000_0000_4000_8000_0000_0000_0001;
pub(crate) const GOLD: u128 = 0xf500_0000_0000_4000_8000_0000_0000_0002;
pub(crate) const CRATE: u128 = 0xf600_0000_0000_4000_8000_0000_0000_0001;
pub(crate) const CRATES: u128 = 0xf600_0000_0000_4000_8000_0000_0000_0002;
pub(crate) const MANIFEST_EVEN: u128 = 0xf700_0000_0000_4000_8000_0000_0000_0001;
pub(crate) const MANIFEST_CUSTOM: u128 = 0xf700_0000_0000_4000_8000_0000_0000_0002;
pub(crate) const MANIFEST_RANDOM: u128 = 0xf700_0000_0000_4000_8000_0000_0000_0003;

const RECORDS: &str = "libs/foundry/records/";

/// Returns the text form of a fixture GUID.
pub(crate) fn guid(value: u128) -> String {
    let text = format!("{value:032x}");
    format!(
        "{}-{}-{}-{}-{}",
        &text[0..8],
        &text[8..12],
        &text[12..16],
        &text[16..20],
        &text[20..32]
    )
}

pub(super) fn record(guid: u128, type_name: &str, name: &str, path: &str) -> TestRecord {
    TestRecord::new(
        guid,
        type_name,
        format!("{type_name}.{name}"),
        format!("{RECORDS}{path}.xml"),
    )
}

pub(super) const fn reference(guid: u128) -> TestValue {
    TestValue::Reference(guid)
}

pub(super) const fn int(value: i32) -> TestValue {
    TestValue::Int32(value)
}

pub(super) const fn single(value: f32) -> TestValue {
    TestValue::Single(value)
}

pub(super) const fn array(values: Vec<TestValue>) -> TestValue {
    TestValue::Array(values)
}

pub(super) const fn structure(instance: TestInstance) -> TestValue {
    TestValue::Struct(instance)
}

fn resource_content(resource: u128, min: i32, max: i32, container: i32) -> TestValue {
    structure(
        TestInstance::new("HaulingOrderContent_Resource")
            .value("resource", reference(resource))
            .value("minSCU", int(min))
            .value("maxSCU", int(max))
            .value("maxContainerSize", int(container)),
    )
}

fn pieces(kind: &str, field: &str, target: u128, min: i32, max: i32) -> TestValue {
    structure(
        TestInstance::new(kind)
            .value(field, reference(target))
            .value("minAmount", int(min))
            .value("maxAmount", int(max)),
    )
}

fn orders_property(name: &str, content: Vec<TestValue>) -> TestValue {
    structure(
        TestInstance::new("MissionProperty")
            .value("missionVariableName", TestValue::text(name))
            .value("extendedTextToken", TestValue::text(""))
            .value(
                "value",
                structure(
                    TestInstance::new("MissionPropertyValue_HaulingOrders")
                        .value("haulingOrderContent", array(content)),
                ),
            ),
    )
}

fn flag_property() -> TestValue {
    structure(
        TestInstance::new("MissionProperty")
            .value("missionVariableName", TestValue::text("Flag"))
            .value("extendedTextToken", TestValue::text("Flag"))
            .value(
                "value",
                structure(
                    TestInstance::new("MissionPropertyValue_Boolean")
                        .value("value", TestValue::Bool(true)),
                ),
            ),
    )
}

fn overrides(properties: Vec<TestValue>) -> TestValue {
    structure(
        TestInstance::new("ContractParamOverrides").value("propertyOverrides", array(properties)),
    )
}

fn contract(id: u128, name: &str, template: Option<u128>, params: TestValue) -> TestInstance {
    let contract = TestInstance::new("CareerContract")
        .value("id", TestValue::Guid(id))
        .value("debugName", TestValue::text(name))
        .value("paramOverrides", params);
    match template {
        Some(template) => contract.value("template", reference(template)),
        None => contract,
    }
}

fn either() -> TestValue {
    let option = |orders: Vec<TestValue>| {
        structure(TestInstance::new("HaulingOrder_OrOption_And").value("orders", array(orders)))
    };
    structure(TestInstance::new("HaulingOrderContent_Or").value(
        "options",
        array(vec![
            option(vec![resource_content(SCRAP, 10, 10, -1)]),
            option(vec![
                resource_content(GOLD, 5, 5, -1),
                pieces(
                    "HaulingOrderContent_EntityClass",
                    "entityClass",
                    CRATE,
                    2,
                    0,
                ),
            ]),
        ]),
    ))
}

fn near_contract() -> TestValue {
    let sub = structure(
        TestInstance::new("SubContract")
            .value("id", TestValue::Guid(SUB))
            .value("debugName", TestValue::text("Sub_Haul"))
            .value(
                "paramOverrides",
                overrides(vec![orders_property("HaulingOverride", vec![either()])]),
            ),
    );
    let content = vec![
        resource_content(SCRAP, 4, 8, 16),
        pieces(
            "HaulingOrderContent_EntityClass",
            "entityClass",
            CRATE,
            3,
            0,
        ),
        pieces(
            "HaulingOrderContent_EntityClasses",
            "haulingEntityClasses",
            CRATES,
            1,
            4,
        ),
        pieces("HaulingOrderContent_MissionItem", "item", NEAR, 1, 0),
    ];
    let params = overrides(vec![orders_property("HaulingOverride", content)]);
    structure(
        contract(NEAR, "Haul_Near", Some(TEMPLATE), params).value("subContracts", array(vec![sub])),
    )
}

fn empty_handler() -> TestInstance {
    TestInstance::new("ContractGeneratorHandler_Career")
        .value("debugName", TestValue::text("Empty_Career"))
        .value(
            "contracts",
            array(vec![structure(contract(
                NOTHING,
                "No_Haul",
                None,
                overrides(vec![]),
            ))]),
        )
}

fn unnamed_handler() -> TestInstance {
    TestInstance::new("ContractGeneratorHandler_Career")
        .value("debugName", TestValue::text("Unnamed_Career"))
        .value(
            "contractParams",
            overrides(vec![orders_property(
                "",
                vec![resource_content(SCRAP, 3, 3, -1)],
            )]),
        )
        .value(
            "contracts",
            array(vec![structure(contract(
                NAMELESS,
                "Unnamed_Haul",
                None,
                overrides(vec![orders_property(
                    "",
                    vec![resource_content(GOLD, 4, 4, -1)],
                )]),
            ))]),
        )
}

fn generator() -> TestRecord {
    let contracts = vec![
        near_contract(),
        structure(contract(
            FAR,
            "Haul_Far",
            Some(TEMPLATE),
            overrides(vec![flag_property()]),
        )),
        structure(contract(
            PLAIN,
            "Haul_Plain",
            Some(TEMPLATE_PLAIN),
            overrides(vec![]),
        )),
        structure(
            contract(BROKERED, "Haul_Brokered", None, overrides(vec![]))
                .value("missionBrokerEntry", reference(BROKER)),
        ),
    ];
    let handler = TestInstance::new("ContractGeneratorHandler_Career")
        .value("debugName", TestValue::text("Haul_Career"))
        .value(
            "contractParams",
            overrides(vec![
                orders_property("HaulingOverride", vec![resource_content(GOLD, 1, 1, -1)]),
                orders_property("HandlerOnly", vec![resource_content(GOLD, 2, 2, -1)]),
            ]),
        )
        .value("contracts", array(contracts));
    record(
        GENERATOR,
        "ContractGenerator",
        "Haul_Career",
        "contracts/contractgenerator/haul_career",
    )
    .value(
        "generators",
        array(vec![
            structure(handler),
            structure(empty_handler()),
            structure(unnamed_handler()),
        ]),
    )
}

fn objective(orders: Vec<TestValue>, handler: &str) -> TestValue {
    structure(TestInstance::new("ObjectiveToken").value(
        "objectiveHandler",
        structure(TestInstance::new(handler).value("haulingOrders", array(orders))),
    ))
}

fn objective_orders() -> Vec<TestValue> {
    vec![
        structure(
            TestInstance::new("HaulingOrder_Resource")
                .value("resource", reference(SCRAP))
                .value("minSCU", int(1))
                .value("maxSCU", int(0))
                .value("maxContainerSize", int(-1)),
        ),
        pieces("HaulingOrder_EntityClass", "entityClass", CRATE, 12, 12),
        pieces("HaulingOrder_MissionItem", "item", NEAR, 1, 0),
        structure(
            TestInstance::new("HaulingOrder_Property")
                .value("haulingOrdersProperty", reference(TEMPLATE)),
        ),
    ]
}

fn database() -> DataForgeBuilder {
    let property = orders_property("HaulingOverride", vec![resource_content(GOLD, 7, 9, 8)]);
    [
        generator(),
        record(
            TEMPLATE,
            "ContractTemplate",
            "haulcargo",
            "contracts/contracttemplates/haulcargo",
        )
        .value("contractProperties", array(vec![property]))
        .value(
            "objectiveTokens",
            array(vec![
                objective(objective_orders(), "ObjectiveHandler_Hauling"),
                objective(vec![resource_content_order()], "ObjectiveHandler_Other"),
            ]),
        ),
        record(
            TEMPLATE_PLAIN,
            "ContractTemplate",
            "plain",
            "contracts/contracttemplates/plain",
        ),
        record(
            BROKER,
            "MissionBrokerEntry",
            "Covalex_HaulCargo",
            "missionbroker/covalex_haulcargo",
        )
        .value(
            "objectiveTokens",
            array(vec![objective(
                vec![resource_content_order()],
                "ObjectiveHandler_Hauling",
            )]),
        ),
        manifest_even(),
        manifest_custom(),
        manifest_random(),
        manifest_random(),
        record(SCRAP, "ResourceType", "Scrap", "resourcetypedatabase/scrap"),
        record(GOLD, "ResourceType", "Gold", "resourcetypedatabase/gold"),
        record(CRATE, "Placeholder", "crate", "entities/crate"),
        record(CRATES, "Placeholder", "crates", "entities/crates"),
    ]
    .into_iter()
    .fold(
        structures(DataForgeBuilder::new().version(8)),
        DataForgeBuilder::record,
    )
}

fn resource_content_order() -> TestValue {
    structure(
        TestInstance::new("HaulingOrder_Resource")
            .value("resource", reference(GOLD))
            .value("minSCU", int(2))
            .value("maxSCU", int(6))
            .value("maxContainerSize", int(32)),
    )
}

/// Writes the fixture Archive into `dir` and returns its path.
pub(crate) fn write_archive(dir: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let path = dir.join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data\\Game2.dcb", database().build()).encrypted())
        .entry(TestEntry::zstd(
            "Data\\Localization\\english\\global.ini",
            GlobalIniBuilder::new().text("title", "Haul").build(),
        ))
        .write_to(&path)?;
    Ok(path)
}
