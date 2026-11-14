// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{Filter, build_catalog};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as archive, DcbOrigin, TestResult, at, guid, text_row};
use crate::health_origin_assertions as proof;

const CONTEXT: u128 = 0x0010_7307;
const SCOPE: u128 = 0x0010_7308;
const MISSING: u128 = 0x0010_7309;
const CONTEXT_TYPE: &str = "SReputationContextUI";
const SCOPE_CONTEXT: &str = "SReputationScopeContextUI";
const SCOPE_TYPE: &str = "SReputationScopeParams";
const PROPERTY_TYPE: &str = "SReputationContextBBPropertyParams";
const BODY_TYPE: &str = "SBBDynamicPropertyBase";
const LOCALE_TYPE: &str = "SBBDynamicPropertyLocString";
const FLOAT_TYPE: &str = "SBBDynamicPropertyFloat";
const CUSTOM_TYPE: &str = "FixtureContextProperty";
const SORT_TYPE: &str = "EReputationSortOrderScope";
const SORT_LABEL: &str = "FixtureFutureSortOrder";

#[test]
fn a_context_keeps_ordered_scope_slots_and_the_owned_display_properties() -> TestResult {
    let (_folder, input) = archive::open(&database(), DcbOrigin::Encrypted)?;
    let filter = Filter::new("*/reputation/contexts/*");
    let mut options = archive::options();
    options.filter = Some(&filter);
    let built = build_catalog(&input, &options)?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let context = at(&catalog, "/definitions/reputationContexts/rows/0")?;
    assert!(catalog.get("reputationContexts").is_none());
    proof::present(context, "guid", &json!(guid(CONTEXT)), Some(true))?;
    proof::present(context, "sortOrderScope", &json!(SORT_LABEL), Some(true))?;
    let primary = at(context, "/primaryScopeContext/value")?;
    assert_eq!(at(primary, "/scope/value/guid/value")?, &json!(guid(SCOPE)));
    let slots = proof::rows(context, "/scopeContextList/value")?;
    let order = slots
        .iter()
        .map(|row| at(row, "/value/scope/value/guid/value"))
        .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(
        order,
        vec![
            &json!(guid(SCOPE)),
            &json!(guid(MISSING)),
            &json!(guid(SCOPE))
        ]
    );
    assert_eq!(
        at(
            context,
            "/scopeContextList/value/1/value/scope/value/status"
        )?,
        &json!("dangling")
    );
    assert_eq!(
        proof::rows(&catalog, "/definitions/reputationScopes/rows")?.len(),
        0
    );
    verify_display_properties(&catalog, primary)?;
    proof::closure(&catalog)
}

fn verify_display_properties(catalog: &Value, primary: &Value) -> TestResult<()> {
    let locale = at(primary, "/propertiesBb/value/0/value/dynamicProperty/value")?;
    assert_eq!(
        at(locale, "/kind")?,
        &json!("localized"),
        "the authored Locale body"
    );
    let reference = proof::cell(locale, "value", "present")?;
    let text = text_row(catalog, "gameTexts", at(reference, "/value")?)?;
    assert_eq!(
        at(text, "/key/value")?,
        &json!("@shared"),
        "the original locale key"
    );
    let float = at(primary, "/propertiesBb/value/2/value/dynamicProperty/value")?;
    assert_eq!(
        at(float, "/kind")?,
        &json!("float"),
        "typed original Single"
    );
    proof::present(
        float,
        "value",
        &json!({"rawBits":"80000000","finite":-0.0}),
        Some(true),
    )?;
    let raw = at(primary, "/propertiesBb/value/1/value/dynamicProperty/value")?;
    proof::present(raw, "sourceType", &json!(CUSTOM_TYPE), Some(true))?;
    assert_eq!(
        at(raw, "/authoredFields/values/0/value/value/value/rawBits")?,
        &json!("7fc01074"),
        "the actual unprojected nonfinite Single"
    );
    Ok(())
}

fn database() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .enumeration(SORT_TYPE, [SORT_LABEL])
        .structure(
            TestStruct::new(CONTEXT_TYPE)
                .field("sortOrderScope", TestType::Enum(SORT_TYPE.to_owned()))
                .field(
                    "primaryScopeContext",
                    TestType::Class(SCOPE_CONTEXT.to_owned()),
                )
                .array(
                    "scopeContextList",
                    TestType::Class(SCOPE_CONTEXT.to_owned()),
                ),
        )
        .structure(
            TestStruct::new(SCOPE_CONTEXT)
                .field("scope", TestType::ReferenceTo(SCOPE_TYPE.to_owned()))
                .array("propertiesBB", TestType::Class(PROPERTY_TYPE.to_owned())),
        )
        .structure(
            TestStruct::new(PROPERTY_TYPE)
                .field("name", TestType::String)
                .field(
                    "dynamicProperty",
                    TestType::StrongPointer(BODY_TYPE.to_owned()),
                ),
        )
        .structure(TestStruct::new(BODY_TYPE))
        .structure(
            TestStruct::new(LOCALE_TYPE)
                .parent(BODY_TYPE)
                .field("value", TestType::Locale),
        )
        .structure(
            TestStruct::new(CUSTOM_TYPE)
                .parent(BODY_TYPE)
                .field("value", TestType::Single),
        )
        .structure(
            TestStruct::new(FLOAT_TYPE)
                .parent(BODY_TYPE)
                .field("value", TestType::Single),
        )
        .structure(TestStruct::new(SCOPE_TYPE))
        .record(TestRecord::new(
            SCOPE,
            SCOPE_TYPE,
            "SReputationScopeParams.Fixture",
            "libs/foundry/records/reputation/scopes/fixture.xml",
        ))
        .record(context())
}

fn context() -> TestRecord {
    let locale = TestInstance::new(LOCALE_TYPE).value("value", TestValue::text("@shared"));
    let raw = TestInstance::new(CUSTOM_TYPE)
        .value("value", TestValue::Single(f32::from_bits(0x7fc0_1074)));
    TestRecord::new(
        CONTEXT,
        CONTEXT_TYPE,
        "SReputationContextUI.Fixture",
        "libs/foundry/records/reputation/contexts/fixture.xml",
    )
    .value("sortOrderScope", TestValue::text(SORT_LABEL))
    .value(
        "primaryScopeContext",
        scope_context(
            SCOPE,
            vec![
                property("label", locale),
                property("raw", raw),
                property(
                    "scale",
                    TestInstance::new(FLOAT_TYPE).value("value", TestValue::Single(-0.0)),
                ),
            ],
        ),
    )
    .value(
        "scopeContextList",
        TestValue::Array(
            [SCOPE, MISSING, SCOPE]
                .into_iter()
                .map(|guid| scope_context(guid, Vec::new()))
                .collect(),
        ),
    )
}

fn scope_context(guid: u128, properties: Vec<TestValue>) -> TestValue {
    TestValue::Struct(
        TestInstance::new(SCOPE_CONTEXT)
            .value("scope", TestValue::Reference(guid))
            .value("propertiesBB", TestValue::Array(properties)),
    )
}

fn property(name: &str, body: TestInstance) -> TestValue {
    TestValue::Struct(
        TestInstance::new(PROPERTY_TYPE)
            .value("name", TestValue::text(name))
            .value("dynamicProperty", TestValue::Struct(body)),
    )
}
