// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Language;
use p4kx_testkit::{
    DataForgeBuilder, GlobalIniBuilder, TestEntry, TestInstance, TestRecord, TestValue,
};

use super::super::{records, structures};
use crate::health_origin_fixture::{Fixture, Input, TestResult};

#[path = "title_origin_source.rs"]
mod source;

const GENERATOR: u128 = 0x7102_0000_0000_4000_8000_0000_0001;
const TEMPLATE: u128 = 0x7102_0000_0000_4000_8000_0000_0002;
const BROKER: u128 = 0x7102_0000_0000_4000_8000_0000_0003;
pub(super) const CONTRACT: u128 = 0x7102_0000_0000_4000_8000_0000_0004;
pub(super) const CHILD: u128 = 0x7102_0000_0000_4000_8000_0000_0005;

pub(super) const TITLE: &str = "@title_origin";
pub(super) const DESCRIPTION: &str = "@description_origin";
const PARENT_PIECE: &str = "@piece_parent";
const CHILD_PIECE: &str = "@piece_child";
const NESTED: &str = "@piece_nested";
const REJECTED: &str = "@LOC_PLACEHOLDER";
const LATER: &str = "@later_title";
const BROKER_TITLE: &str = "@broker_title";
const TEMPLATE_TITLE: &str = "@template_title";

const ENGLISH_TITLE: &str = "Inspect ~mission(Piece)";
pub(super) const GERMAN_TITLE: &str = "Pruefen";
const ENGLISH_DESCRIPTION: &str = r"  Verify\nData  ";
pub(super) const GERMAN_DESCRIPTION: &str = "Daten";

#[derive(Clone, Copy, Debug)]
pub(super) struct Site {
    pub(super) record: u32,
    pub(super) ordinal: u64,
}

pub(super) struct Original {
    pub(super) handler: Site,
    pub(super) contract: Site,
    pub(super) child: Site,
    pub(super) title: Site,
    pub(super) description: Site,
    pub(super) parent_piece: Site,
    pub(super) child_piece: Site,
    pub(super) nested: Site,
    pub(super) rejected: Site,
    pub(super) later: Site,
    pub(super) template_title: Site,
    pub(super) broker_title: Site,
}

pub(super) fn open(input: Input) -> TestResult<(Fixture, Original)> {
    let builder = structures::structures(
        DataForgeBuilder::new()
            .version(8)
            .enumeration("ECurrency", ["UEC"]),
    );
    let bytes = builder
        .record(generator())
        .record(template())
        .record(broker())
        .build();
    let fixture = Fixture::open(&bytes, input, languages())?;
    let original = source::original(&fixture, input)?;
    Ok((fixture, original))
}

fn generator() -> TestRecord {
    let child = TestInstance::new("SubContract")
        .value("id", TestValue::Guid(CHILD))
        .value(
            "paramOverrides",
            parameters(
                vec![title(REJECTED), title(LATER)],
                vec![records::property(
                    "pIECe",
                    records::string_hash(CHILD_PIECE),
                )],
            ),
        );
    let contract = TestInstance::new("CareerContract")
        .value("id", TestValue::Guid(CONTRACT))
        .value("template", TestValue::Reference(TEMPLATE))
        .value("missionBrokerEntry", TestValue::Reference(BROKER))
        .value(
            "paramOverrides",
            parameters(
                vec![title(TITLE)],
                vec![
                    records::property("Piece", records::string_hash(PARENT_PIECE)),
                    records::property("Nested", records::string_hash(NESTED)),
                ],
            ),
        )
        .value(
            "subContracts",
            TestValue::Array(vec![TestValue::Struct(child)]),
        );
    let handler = TestInstance::new("ContractGeneratorHandler_List").value(
        "contracts",
        TestValue::Array(vec![TestValue::Struct(contract)]),
    );
    TestRecord::new(
        GENERATOR,
        "ContractGenerator",
        "ContractGenerator.Origin",
        "libs/foundry/records/contracts/contractgenerator/title_origin.xml",
    )
    .value(
        "generators",
        TestValue::Array(vec![TestValue::Struct(handler)]),
    )
}

fn parameters(titles: Vec<TestValue>, properties: Vec<TestValue>) -> TestValue {
    TestValue::Struct(
        TestInstance::new("ContractParamOverrides")
            .value("stringParamOverrides", TestValue::Array(titles))
            .value("propertyOverrides", TestValue::Array(properties)),
    )
}

fn title(key: &str) -> TestValue {
    TestValue::Struct(
        TestInstance::new("ContractStringParam")
            .value("param", TestValue::text("Title"))
            .value("value", TestValue::text(key)),
    )
}

fn template() -> TestRecord {
    let title = TestInstance::new("LocID").value("value", TestValue::text(TEMPLATE_TITLE));
    let display = TestInstance::new("ContractDisplayInfo").value(
        "displayString",
        TestValue::Array(vec![TestValue::Struct(title)]),
    );
    TestRecord::new(
        TEMPLATE,
        "ContractTemplate",
        "ContractTemplate.Origin",
        "libs/foundry/records/contracts/contracttemplates/title_origin.xml",
    )
    .value("contractDisplayInfo", TestValue::Struct(display))
}

fn broker() -> TestRecord {
    TestRecord::new(
        BROKER,
        "MissionBrokerEntry",
        "MissionBrokerEntry.Origin",
        "libs/foundry/records/missionbroker/title_origin.xml",
    )
    .value("title", TestValue::text(BROKER_TITLE))
    .value("description", TestValue::text(DESCRIPTION))
}

fn languages() -> Vec<TestEntry> {
    let english = GlobalIniBuilder::new()
        .text("title_origin", "")
        .text("title_origin,P", ENGLISH_TITLE)
        .text("description_origin", ENGLISH_DESCRIPTION)
        .text("piece_parent", "Parent ~mission(Nested)")
        .text("piece_child", "Child ~mission(Nested)")
        .text("piece_nested", "ready")
        .text("later_title", "Later declaration")
        .text("broker_title", "Broker fallback")
        .text("template_title", "Template fallback");
    let german = GlobalIniBuilder::new()
        .text("title_origin", GERMAN_TITLE)
        .text("description_origin", GERMAN_DESCRIPTION);
    let french = GlobalIniBuilder::new().text("unrelated_key", "Autre");
    vec![
        TestEntry::stored(Language::English.global_ini_path(), english.build()).encrypted(),
        TestEntry::stored(Language::German.global_ini_path(), german.build()),
        TestEntry::stored(Language::French.global_ini_path(), french.build()).encrypted(),
    ]
}
