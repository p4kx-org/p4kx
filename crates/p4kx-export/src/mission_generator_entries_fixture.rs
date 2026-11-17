// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

pub(super) const ROOT: u128 = 0x1000_0000_0000_4000_8000_0000_0000_0001;
pub(super) const OUTSIDE: u128 = 0x1000_0000_0000_4000_8000_0000_0000_0002;
const TAG: u128 = 0x4000_0000_0000_4000_8000_0000_0000_0001;
pub(super) const CONTRACT_TEXT: &str = "20000000-0000-4000-8000-000000000001";
pub(super) const INTRO_TEXT: &str = "30000000-0000-4000-8000-000000000003";
const CONTRACT: u128 = 0x2000_0000_0000_4000_8000_0000_0000_0001;
pub(super) const SUB: u128 = 0x3000_0000_0000_4000_8000_0000_0000_0001;
const OUTSIDE_CONTRACT: u128 = 0x2000_0000_0000_4000_8000_0000_0000_0050;
const INTRO: u128 = 0x3000_0000_0000_4000_8000_0000_0000_0003;

fn pointer(name: &str) -> TestType {
    TestType::StrongPointer(name.to_owned())
}

fn contract(id: u128, name: &str) -> TestInstance {
    TestInstance::new("Contract")
        .value("id", TestValue::Guid(id))
        .value("debugName", TestValue::text(name))
}

fn source(guid: u128, name: &str, handler: TestInstance) -> TestRecord {
    TestRecord::new(
        guid,
        "ContractGenerator",
        format!("ContractGenerator.{name}"),
        format!("libs/foundry/records/contracts/contractgenerator/{name}.xml"),
    )
    .value(
        "generators",
        TestValue::Array(vec![TestValue::Struct(handler)]),
    )
}

fn schema() -> [TestStruct; 12] {
    [
        TestStruct::new("ContractGenerator").array("generators", pointer("Handler")),
        TestStruct::new("Handler")
            .array("contracts", pointer("Contract"))
            .array("introContracts", pointer("Contract")),
        TestStruct::new("InvalidHandler")
            .parent("Handler")
            .field("contractParams", TestType::UInt32),
        TestStruct::new("Contract")
            .field("id", TestType::Guid)
            .field("debugName", TestType::String)
            .array("subContracts", pointer("Contract"))
            .array("additionalPrerequisites", pointer("Prerequisite"))
            .field("contractResults", TestType::Class("ResultBlock".to_owned())),
        TestStruct::new("Prerequisite"),
        TestStruct::new("ContractPrerequisite_CompletedContractTags")
            .parent("Prerequisite")
            .field(
                "requiredCompletedContractTags",
                TestType::Class("TagList".to_owned()),
            ),
        TestStruct::new("TagList").array("tags", TestType::Reference),
        TestStruct::new("ResultBlock").array("contractResults", pointer("Result")),
        TestStruct::new("Result"),
        TestStruct::new("ContractResult_CompletionTags")
            .parent("Result")
            .array("missionResults", TestType::Bool)
            .array(
                "completionTags",
                TestType::Class("CompletionTag".to_owned()),
            ),
        TestStruct::new("CompletionTag").field("tag", TestType::Reference),
        TestStruct::new("Tag"),
    ]
}

fn prerequisite() -> TestValue {
    TestValue::Struct(
        TestInstance::new("ContractPrerequisite_CompletedContractTags").value(
            "requiredCompletedContractTags",
            TestValue::Struct(
                TestInstance::new("TagList")
                    .value("tags", TestValue::Array(vec![TestValue::Reference(TAG)])),
            ),
        ),
    )
}

fn root_contract(options: Options) -> TestInstance {
    let mut parent = contract(CONTRACT, "Root");
    if options.intro.parent {
        parent = parent.value(
            "additionalPrerequisites",
            TestValue::Array(vec![prerequisite()]),
        );
    }
    let mut sub = contract(SUB, "Sub");
    if options.intro.sub {
        sub = sub.value(
            "additionalPrerequisites",
            TestValue::Array(vec![prerequisite()]),
        );
    }
    parent.value(
        "subContracts",
        TestValue::Array(vec![
            TestValue::Struct(sub),
            TestValue::Struct(contract(SUB + 1, "dev_temp")),
        ]),
    )
}

fn root_handler(options: Options) -> TestInstance {
    let mut handler = TestInstance::new(if options.root_handler == HandlerState::Malformed {
        "InvalidHandler"
    } else {
        "Handler"
    });
    if options.root_handler == HandlerState::Malformed {
        handler = handler.value("contractParams", TestValue::UInt32(3));
    }
    handler.value(
        "contracts",
        TestValue::Array(vec![
            TestValue::Struct(root_contract(options)),
            TestValue::Null,
            TestValue::Struct(contract(CONTRACT + 1, "dev_temp")),
        ]),
    )
}

fn intro_contract(success: bool) -> TestInstance {
    let grant = TestInstance::new("ContractResult_CompletionTags")
        .value(
            "missionResults",
            TestValue::Array(vec![TestValue::Bool(success)]),
        )
        .value(
            "completionTags",
            TestValue::Array(vec![TestValue::Struct(
                TestInstance::new("CompletionTag").value("tag", TestValue::Reference(TAG)),
            )]),
        );
    contract(INTRO, "Intro").value(
        "contractResults",
        TestValue::Struct(TestInstance::new("ResultBlock").value(
            "contractResults",
            TestValue::Array(vec![TestValue::Struct(grant)]),
        )),
    )
}

fn outside_handler(options: Options) -> TestInstance {
    let handler = TestInstance::new(if options.outside_handler == HandlerState::Malformed {
        "InvalidHandler"
    } else {
        "Handler"
    });
    let handler = if options.outside_handler == HandlerState::Malformed {
        handler.value("contractParams", TestValue::UInt32(3))
    } else {
        handler
    };
    let mut outside = contract(
        if options.include_collision {
            CONTRACT
        } else {
            OUTSIDE_CONTRACT
        },
        "OutsideContract",
    );
    if options.outside_subcontract {
        outside = outside.value(
            "subContracts",
            TestValue::Array(vec![TestValue::Struct(contract(SUB, "OutsideSub"))]),
        );
    }
    handler
        .value(
            "contracts",
            TestValue::Array(vec![TestValue::Struct(outside)]),
        )
        .value(
            "introContracts",
            TestValue::Array(vec![
                TestValue::Null,
                TestValue::Struct(intro_contract(options.successful_grant)),
                TestValue::Struct(contract(0, "NoIdentity")),
            ]),
        )
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum HandlerState {
    #[default]
    Valid,
    Malformed,
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum UnrelatedOwner {
    #[default]
    Absent,
    Malformed,
}

#[derive(Clone, Copy)]
pub(super) struct IntroRequirements {
    pub(super) parent: bool,
    pub(super) sub: bool,
}

impl Default for IntroRequirements {
    fn default() -> Self {
        Self {
            parent: true,
            sub: false,
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct Options {
    pub(super) outside_handler: HandlerState,
    pub(super) unrelated_owner: UnrelatedOwner,
    pub(super) successful_grant: bool,
    pub(super) include_collision: bool,
    pub(super) outside_instance: Option<u16>,
    pub(super) root_handler: HandlerState,
    pub(super) outside_subcontract: bool,
    pub(super) intro: IntroRequirements,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            outside_handler: HandlerState::Valid,
            unrelated_owner: UnrelatedOwner::Absent,
            successful_grant: true,
            include_collision: true,
            outside_instance: None,
            root_handler: HandlerState::Valid,
            outside_subcontract: false,
            intro: IntroRequirements::default(),
        }
    }
}

pub(super) fn database(broken_outside: bool, unrelated_invalid: bool) -> Vec<u8> {
    database_with(Options {
        outside_handler: if broken_outside {
            HandlerState::Malformed
        } else {
            HandlerState::Valid
        },
        unrelated_owner: if unrelated_invalid {
            UnrelatedOwner::Malformed
        } else {
            UnrelatedOwner::Absent
        },
        ..Options::default()
    })
}

pub(super) fn database_with(options: Options) -> Vec<u8> {
    let mut outside = source(OUTSIDE, "outside", outside_handler(options));
    if let Some(index) = options.outside_instance {
        outside = outside.with_instance_index(index);
    }
    let builder = schema()
        .into_iter()
        .fold(
            DataForgeBuilder::new().version(8),
            DataForgeBuilder::structure,
        )
        .record(TestRecord::new(
            TAG,
            "Tag",
            "Tag.Access",
            "libs/foundry/records/tags/access.xml",
        ))
        .record(source(ROOT, "root", root_handler(options)))
        .record(outside);
    let builder = if options.unrelated_owner == UnrelatedOwner::Malformed {
        builder.record(source(
            ROOT + 100,
            "unrelated",
            TestInstance::new("InvalidHandler")
                .value("contractParams", TestValue::UInt32(3))
                .value(
                    "contracts",
                    TestValue::Array(vec![TestValue::Struct(contract(
                        CONTRACT + 100,
                        "Unrelated",
                    ))]),
                ),
        ))
    } else {
        builder
    };
    builder.build()
}
