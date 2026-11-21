// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestStruct, TestType, TestValue};

const BASE: &str = "DataForgeComponentParams";
const HACKABLE: &str = "HackableParams";
const AUDIO_TRIGGER: &str = "GlobalResourceAudio";
const AUDIO_RTPC: &str = "AudioRtpc";

pub(super) fn schema(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder
        .structure(TestStruct::new(AUDIO_TRIGGER).field("audioTrigger", TestType::String))
        .structure(
            TestStruct::new(AUDIO_RTPC)
                .field("rtpc", TestType::String)
                .field("fixtureTuning", TestType::UInt16),
        )
        .structure(
            TestStruct::new(HACKABLE)
                .parent(BASE)
                .field("baseDuration", TestType::Single)
                .field("baseErrorChance", TestType::Single)
                .field("maxPauseDuration", TestType::Single)
                .field("numErrorChecks", TestType::UInt32)
                .field("resetAfterHackSuccess", TestType::Bool)
                .field("updateProgressDeltaTime", TestType::Single)
                .field(
                    "audioTriggerStart",
                    TestType::Class(AUDIO_TRIGGER.to_owned()),
                )
                .field(
                    "audioRtpcOverallProgress",
                    TestType::Class(AUDIO_RTPC.to_owned()),
                ),
        )
}

pub(super) fn component() -> TestValue {
    TestValue::Struct(
        TestInstance::new(HACKABLE)
            .value("baseDuration", TestValue::Single(2.0))
            .value("baseErrorChance", TestValue::Single(-0.0))
            .value("maxPauseDuration", TestValue::Single(3.0))
            .value("numErrorChecks", TestValue::UInt32(0))
            .value("resetAfterHackSuccess", TestValue::Bool(false))
            .value("updateProgressDeltaTime", TestValue::Single(0.125))
            .value(
                "audioTriggerStart",
                TestValue::Struct(
                    TestInstance::new(AUDIO_TRIGGER).value("audioTrigger", TestValue::text("")),
                ),
            )
            .value(
                "audioRtpcOverallProgress",
                TestValue::Struct(
                    TestInstance::new(AUDIO_RTPC)
                        .value("rtpc", TestValue::text("FixtureProgress"))
                        .value("fixtureTuning", TestValue::UInt16(17)),
                ),
            ),
    )
}
