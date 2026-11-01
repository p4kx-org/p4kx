// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Records whose location query holds a condition of a struct the catalog does not know.

use p4kx_testkit::{TestInstance, TestRecord, TestValue};

use super::template_values as values;
use super::{
    CONTRACT_UNKNOWN_CONDITION, GENERATOR_UNKNOWN_CONDITION, UNKNOWN_LOCATION_AFTER_KNOWN,
    UNKNOWN_LOCATION_CONDITION, array, overrides, text,
};

/// A template with a contract property whose only condition is of an unknown struct.
pub(super) fn template() -> TestRecord {
    template_with(
        UNKNOWN_LOCATION_CONDITION,
        "Unknown_Location_Condition",
        false,
    )
}

/// A template with a contract property whose known condition is followed by an unknown one.
pub(super) fn template_after_known() -> TestRecord {
    template_with(
        UNKNOWN_LOCATION_AFTER_KNOWN,
        "Unknown_Location_After_Known",
        true,
    )
}

fn template_with(guid: u128, name: &str, known_first: bool) -> TestRecord {
    TestRecord::new(
        guid,
        "ContractTemplate",
        format!("ContractTemplate.{name}"),
        format!(
            "libs/foundry/records/contracts/contracttemplates/{}.xml",
            name.to_lowercase()
        ),
    )
    .value(
        "contractProperties",
        array(vec![values::mystery_location("Place_BP", known_first)]),
    )
}

/// A generator whose contract overrides a location property with an unknown condition.
pub(super) fn generator() -> TestRecord {
    let contract = TestInstance::new("CareerContract")
        .value("id", TestValue::Guid(CONTRACT_UNKNOWN_CONDITION))
        .value("debugName", text("Unknown_Location_Contract"))
        .value(
            "paramOverrides",
            overrides(vec![values::mystery_location("Place_BP", false)]),
        );
    let handler = TestInstance::new("ContractGeneratorHandler_Career")
        .value("debugName", text("Unknown_Location_Career"))
        .value("contracts", array(vec![TestValue::Struct(contract)]));
    TestRecord::new(
        GENERATOR_UNKNOWN_CONDITION,
        "ContractGenerator",
        "ContractGenerator.Unknown_Location",
        "libs/foundry/records/contracts/contractgenerator/unknown_location.xml",
    )
    .value("generators", array(vec![TestValue::Struct(handler)]))
}
