// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};
use serde_json::{Value, json};

use crate::health_origin_assertions as origin;
use crate::health_origin_fixture::{Fixture, Input, TestResult};

const RESULT: &str = "ContractResult_CompletionTags";
const FIELD: &str = "splitPointsForParty";
const RESULTS: &str = "contractResults";

#[test]
fn completion_tag_results_preserve_declared_true_false_and_physical_occurrences() -> TestResult {
    let db = database(
        Some(TestType::Bool),
        Some([TestValue::Bool(true), TestValue::Bool(false)]),
    );
    for input in [Input::Plain, Input::Encrypted, Input::Standalone] {
        let fixture = Fixture::open(&db, input, Vec::new())?;
        let (_, catalog) = fixture.diagnostic()?;
        let definition = definition(&catalog)?;
        let [first, second] = origin::rows(definition, "/completionTagResults")? else {
            return Err("expected the two original completion-tag results".into());
        };
        assert_ne!(
            origin::at(first, "/source/valueIndex")?,
            origin::at(second, "/source/valueIndex")?,
            "two source slots retain separate identities"
        );
        for (row, expected) in [(first, true), (second, false)] {
            let cell = origin::present(row, FIELD, &json!(expected), input.marker())?;
            let value_index = raw_value_index(definition, row)?.ok_or("original Bool field")?;
            let raw_value = origin::at(definition, &format!("/source/values/{value_index}/value"))?;
            assert_eq!(
                origin::at(raw_value, "/value/value")?,
                &json!(expected),
                "typed and original Boolean values agree"
            );
            assert_use(&catalog, cell, value_index, "$value", "payload")?;
        }
        origin::closure(&catalog)?;
    }
    Ok(())
}

#[test]
fn completion_tag_results_distinguish_absent_and_unsupported_declarations() -> TestResult {
    let cases = [
        (None, None, "absent"),
        (
            Some(TestType::Int32),
            Some([TestValue::Int32(1), TestValue::Int32(0)]),
            "unsupported",
        ),
    ];
    for (declaration, values, state) in cases {
        let data = database(declaration, values);
        let fixture = Fixture::open(&data, Input::Encrypted, Vec::new())?;
        let (_, catalog) = fixture.diagnostic()?;
        let definition = definition(&catalog)?;
        let [first, second] = origin::rows(definition, "/completionTagResults")? else {
            return Err("expected the two original completion-tag results".into());
        };
        for row in [first, second] {
            let cell = origin::cell(row, FIELD, state)?;
            assert_eq!(
                origin::at(cell, "/containsEncryptedSource")?,
                true,
                "the original declarations belong to encrypted DataForge"
            );
            assert_eq!(
                origin::at(cell, "/sourceSetComplete")?,
                true,
                "the original declaration is readable"
            );
            if let Some(value_index) = raw_value_index(definition, row)? {
                assert_eq!(
                    state, "unsupported",
                    "the declaration exists but is incompatible"
                );
                assert_use(&catalog, cell, value_index, "$dataType", "selector")?;
                let raw = origin::at(
                    definition,
                    &format!("/source/values/{value_index}/value/value/type"),
                )?;
                assert_eq!(raw, "number", "the unsupported field remains available");
                assert_diagnostic(&catalog, "unsupportedValue")?;
            } else {
                assert_eq!(state, "absent", "the original survey found no declaration");
                let result_index = result_index(row)?;
                assert_use(&catalog, cell, result_index, "$fields", "selector")?;
            }
        }
        origin::closure(&catalog)?;
    }
    Ok(())
}

#[test]
fn completion_tag_results_preserve_unreadable_bool_storage_and_its_attempt() -> TestResult {
    let db = database(
        Some(TestType::Bool),
        Some([TestValue::Raw(vec![2]), TestValue::Bool(false)]),
    );
    let fixture = Fixture::open(&db, Input::Encrypted, Vec::new())?;
    let (_, catalog) = fixture.diagnostic()?;
    let definition = definition(&catalog)?;
    let [first, second] = origin::rows(definition, "/completionTagResults")? else {
        return Err("expected the two original completion-tag results".into());
    };
    let cell = origin::cell(first, FIELD, "unreadable")?;
    let value_index = raw_value_index(definition, first)?.ok_or("original invalid Bool field")?;
    assert_eq!(
        origin::at(cell, "/containsEncryptedSource")?,
        true,
        "the original declarations belong to encrypted DataForge"
    );
    assert_eq!(
        origin::at(cell, "/sourceSetComplete")?,
        false,
        "the unreadable payload leaves the source set incomplete"
    );
    assert!(
        origin::rows(&catalog, "/provenance/uses")?
            .iter()
            .any(|used| is_use(used, value_index, "$value", "attempted")),
        "the failed Bool read remains in the global source-use inventory"
    );
    assert_diagnostic(&catalog, "unreadableSource")?;
    origin::present(second, FIELD, &json!(false), Some(true))?;
    origin::closure(&catalog)
}

fn assert_diagnostic(catalog: &Value, expected: &str) -> TestResult {
    let diagnostics = origin::rows(catalog, "/exportHealth/diagnostics")?;
    assert!(
        diagnostics.iter().any(|diagnosis| {
            diagnosis["code"] == expected && diagnosis["stage"] == "projection"
        }),
        "the original field has a scoped {expected} projection diagnosis: {diagnostics:?}"
    );
    Ok(())
}

fn definition(catalog: &Value) -> TestResult<&Value> {
    let [row] = origin::rows(catalog, "/definitions/contractGenerators/rows")? else {
        return Err("expected the physical Generator definition".into());
    };
    Ok(row)
}

fn result_index(row: &Value) -> TestResult<&str> {
    Ok(origin::at(row, "/source/valueIndex")?
        .as_str()
        .ok_or("result occurrence index")?)
}

fn raw_value_index<'a>(definition: &'a Value, row: &Value) -> TestResult<Option<&'a str>> {
    let result = origin::at(
        definition,
        &format!("/source/values/{}/value/value", result_index(row)?),
    )?;
    assert_eq!(
        origin::at(result, "/sourceType/value")?,
        RESULT,
        "the link targets its original completion-tag result"
    );
    for field in origin::rows(result, "/fields")? {
        if origin::at(field, "/name/value")? == FIELD {
            return Ok(Some(
                origin::at(field, "/value/valueIndex")?
                    .as_str()
                    .ok_or("original flag value index")?,
            ));
        }
    }
    Ok(None)
}

fn assert_use(catalog: &Value, cell: &Value, ordinal: &str, field: &str, role: &str) -> TestResult {
    assert!(
        origin::uses(catalog, cell)?
            .iter()
            .any(|used| is_use(used, ordinal, field, role)),
        "the flag uses the exact original {field} at occurrence {ordinal}"
    );
    Ok(())
}

fn is_use(used: &Value, ordinal: &str, field: &str, role: &str) -> bool {
    used.pointer("/location/namespace") == Some(&json!("dataforgeValue"))
        && used.pointer("/location/indices") == Some(&json!(["0", ordinal]))
        && used.pointer("/location/field") == Some(&json!(field))
        && used.get("role") == Some(&json!(role))
}

const FIXTURE_BLOCK: &str = "FixtureResultBlock";

fn fixture_contract(contract: &str, id: u128, value: Option<TestValue>) -> TestValue {
    let result = value.map_or_else(
        || TestInstance::new(RESULT),
        |value| TestInstance::new(RESULT).value(FIELD, value),
    );
    let slots = TestValue::Array(vec![TestValue::Struct(result)]);
    TestValue::Struct(
        TestInstance::new(contract)
            .value("id", TestValue::Guid(id))
            .value(
                RESULTS,
                TestValue::Struct(TestInstance::new(FIXTURE_BLOCK).value(RESULTS, slots)),
            ),
    )
}

fn database(declaration: Option<TestType>, values: Option<[TestValue; 2]>) -> Vec<u8> {
    const GENERATOR: &str = "ContractGenerator";
    const HANDLER: &str = "FixtureHandler";
    const CONTRACT: &str = "FixtureContract";
    const BASE: &str = "ContractResult";

    let result = TestStruct::new(RESULT).parent(BASE);
    let result = if let Some(kind) = declaration {
        result.field(FIELD, kind)
    } else {
        result
    };
    let [first, second] = values.map_or([None, None], |values| values.map(Some));
    let handler = TestInstance::new(HANDLER).value(
        "contracts",
        TestValue::Array(vec![
            fixture_contract(CONTRACT, 0x741, first),
            fixture_contract(CONTRACT, 0x741, second),
        ]),
    );
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new(BASE))
        .structure(result)
        .structure(
            TestStruct::new(FIXTURE_BLOCK).array(RESULTS, TestType::StrongPointer(BASE.to_owned())),
        )
        .structure(
            TestStruct::new(CONTRACT)
                .field("id", TestType::Guid)
                .field(RESULTS, TestType::Class(FIXTURE_BLOCK.to_owned())),
        )
        .structure(
            TestStruct::new(HANDLER).array("contracts", TestType::Class(CONTRACT.to_owned())),
        )
        .structure(
            TestStruct::new(GENERATOR).array("generators", TestType::Class(HANDLER.to_owned())),
        )
        .record(
            TestRecord::new(
                0x740,
                GENERATOR,
                "ContractGenerator.FixtureCompletionTags",
                "libs/foundry/records/contracts/contractgenerator/fixture_completion_tags.xml",
            )
            .value(
                "generators",
                TestValue::Array(vec![TestValue::Struct(handler)]),
            ),
        )
        .build()
}
