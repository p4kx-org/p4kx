// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

use crate::health_origin_fixture::{Fixture, Input, TestResult};

use super::source::{self, Original};

use super::super::{flag, number, overrides, sub_contract};

pub(super) const GENERATOR: u128 = 0x73c1;
pub(super) const PARENT: u128 = 0x73c2;
pub(super) const CHILD: u128 = 0x73c3;

#[derive(Clone, Copy)]
pub(super) enum Case {
    Layered,
    FullyOverridden,
    Defaults,
    Missing,
    Unreadable,
}

pub(super) fn open(case: Case, input: Input) -> TestResult<(Fixture, Original)> {
    let mut builder = crate::facts_structs::all().into_iter().fold(
        DataForgeBuilder::new().version(8),
        DataForgeBuilder::structure,
    );
    for structure in extra_structs(case) {
        builder = builder.structure(structure);
    }
    let data = builder.record(generator(case)).build();
    let fixture = Fixture::open(&data, input, Vec::new())?;
    let original = source::read(&fixture, case)?;
    Ok((fixture, original))
}

fn extra_structs(case: Case) -> Vec<TestStruct> {
    let mut availability = TestStruct::new("CooldownAvailability");
    if !matches!(case, Case::Missing) {
        availability = availability.field("hasPersonalCooldown", TestType::Bool);
    }
    for field in [
        "personalCooldownTime",
        "personalCooldownTimeVariation",
        "abandonedCooldownTime",
        "abandonedCooldownTimeVariation",
    ] {
        availability = availability.field(field, TestType::Single);
    }
    vec![
        availability,
        TestStruct::new("ContractHandler_Cooldown")
            .parent("ContractHandler")
            .field("debugName", TestType::String)
            .field(
                "defaultAvailability",
                TestType::Class("CooldownAvailability".to_owned()),
            )
            .field(
                "contractParams",
                TestType::Class("ContractParamOverrides".to_owned()),
            )
            .array("contracts", TestType::Class("CareerContract".to_owned())),
    ]
}

fn generator(case: Case) -> TestRecord {
    let mut parent = TestInstance::new("CareerContract")
        .value("id", TestValue::Guid(PARENT))
        .value("debugName", TestValue::text("Cooldown"));
    if matches!(case, Case::Layered) {
        parent = layered(parent);
    }
    if matches!(case, Case::FullyOverridden) {
        parent = fully_overridden(parent);
    }
    let mut handler = TestInstance::new("ContractHandler_Cooldown")
        .value("debugName", TestValue::text("Cooldown_Career"))
        .value("defaultAvailability", defaults(case))
        .value(
            "contracts",
            TestValue::Array(vec![TestValue::Struct(parent)]),
        );
    if matches!(case, Case::Layered) {
        handler = handler.value(
            "contractParams",
            overrides(
                Vec::new(),
                vec![
                    number("PERSONALCOOLDOWNTIMEVARIATION", 0),
                    number("AbandonedCooldownTime", 15),
                ],
            ),
        );
    }
    TestRecord::new(
        GENERATOR,
        "ContractGenerator",
        "ContractGenerator.Cooldown",
        "libs/foundry/records/contracts/contractgenerator/cooldown_origin.xml",
    )
    .value(
        "generators",
        TestValue::Array(vec![TestValue::Struct(handler)]),
    )
}

fn fully_overridden(parent: TestInstance) -> TestInstance {
    parent.value(
        "paramOverrides",
        overrides(
            vec![
                flag("HasPersonalCooldown", false),
                flag("CanBeShared", true),
                flag("Illegal", false),
                flag("OnceOnly", true),
                flag("AvailableInPrison", false),
                flag("CanReacceptAfterAbandoning", true),
                flag("CanReacceptAfterFailing", false),
                flag("HideInMobiGlas", true),
            ],
            vec![
                number("PersonalCooldownTime", 45),
                number("PersonalCooldownTimeVariation", 0),
                number("AbandonedCooldownTime", 15),
                number("AbandonedCooldownTimeVariation", 1),
                number("MaxPlayersPerInstance", 1),
            ],
        ),
    )
}

fn layered(parent: TestInstance) -> TestInstance {
    let child = sub_contract(CHILD, "CooldownChild").value(
        "intParamOverrides",
        TestValue::Array(vec![
            number("personalcooldowntime", 7),
            number("AbandonedCooldownTime", 0),
        ]),
    );
    parent
        .value(
            "paramOverrides",
            overrides(
                vec![flag("HasPersonalCooldown", false)],
                vec![
                    number("PersonalCooldownTime", 45),
                    number("pERSONALcOOLDOWNtIME", 99),
                    number("AbandonedCooldownTimeVariation", 0),
                ],
            ),
        )
        .value(
            "subContracts",
            TestValue::Array(vec![TestValue::Struct(child)]),
        )
}

fn defaults(case: Case) -> TestValue {
    let flag = match case {
        Case::Unreadable => TestValue::Raw(vec![3]),
        Case::Layered | Case::FullyOverridden | Case::Defaults | Case::Missing => {
            TestValue::Bool(true)
        }
    };
    TestValue::Struct(
        TestInstance::new("CooldownAvailability")
            .value("hasPersonalCooldown", flag)
            .value("personalCooldownTime", TestValue::Single(120.0))
            .value("personalCooldownTimeVariation", TestValue::Single(5.0))
            .value("abandonedCooldownTime", TestValue::Single(30.0))
            .value("abandonedCooldownTimeVariation", TestValue::Single(9.0)),
    )
}
