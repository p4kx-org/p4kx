// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::path::Path;

use p4kx_extraction::{Archive, Language};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, GlobalIniBuilder, TestEntry, TestInstance, TestRecord,
    TestStruct, TestType, TestValue, test_key,
};
use serde_json::{Value, json};

use super::super::ReducedError;
use super::super::contracts;
use super::super::selected_texts::SelectedTexts;
use super::{ContractPosition, PATH, collect};
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection,
};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const GENERATOR: &str = "ContractGenerator";
const HANDLER: &str = "ContractGeneratorHandler_List";
const CONTRACT: &str = "CareerContract";
const CHILD: &str = "SubContract";
const PARAMS: &str = "ContractParamOverrides";
const STRING_PARAM: &str = "ContractStringParam";
const BOOL_PARAM: &str = "ContractBoolParam";

#[test]
fn mission_titles_retain_ordered_physical_links_and_protected_bilingual_closure() -> TestResult {
    let scratch = tempfile::tempdir()?;
    let archive = archive(scratch.path())?;
    let session = session(&archive)?;
    session.parsed(None)?.with_contracts(
        CatalogRecordSelection::All,
        None,
        |mut input| -> TestResult {
            let mut texts = SelectedTexts::default();
            let table = collect(&input, scratch.path(), None, &mut texts)?;
            let table = serde_json::to_value(table)?;
            let texts = serde_json::to_value(texts)?;
            let rows = array(&table, "rows")?;
            assert_eq!(rows.len(), 3, "two parent contracts and one sub contract");
            for (row, original) in rows.iter().zip(&input.rows) {
                assert_eq!(
                    row.get("contract"),
                    Some(&serde_json::to_value(ContractPosition::from(
                        &original.base.source
                    ))?),
                    "the title uses the same physical position as its canonical contract"
                );
                check_text(
                    row,
                    "title",
                    &texts,
                    "Parent title",
                    Some("Auftrag"),
                    "fixture_title",
                )?;
                check_text(
                    row,
                    "description",
                    &texts,
                    "Parent description",
                    None,
                    "fixture_description",
                )?;
            }
            assert_distinct_contract_positions(rows);
            let no_names: &[Value] = &[];
            assert_eq!(array(&texts, "publicNames")?, no_names);
            assert_eq!(array(&texts, "gameTexts")?.len(), 2);

            input.mission_titles.swap(0, 1);
            assert!(matches!(
                collect(&input, scratch.path(), None, &mut SelectedTexts::default()),
                Err(ReducedError::IncompleteScope(PATH))
            ));
            Ok(())
        },
    )??;
    Ok(())
}

fn assert_distinct_contract_positions(rows: &[Value]) {
    assert!(
        rows.iter().any(|row| row
            .pointer("/contract/subContractIndex")
            .is_some_and(Value::is_string)),
        "the sub contract includes its own physical position"
    );
    assert!(
        rows.iter()
            .any(|row| row.pointer("/contract/recordIndex") == Some(&Value::from(1)))
            && rows
                .iter()
                .any(|row| row.pointer("/contract/recordIndex") == Some(&Value::from(0))),
        "the two original Generator owners remain distinguishable"
    );
}

#[test]
fn reduced_contracts_share_title_coordinates_and_keep_layered_choices() -> TestResult {
    let scratch = tempfile::tempdir()?;
    let archive = archive(scratch.path())?;
    session(&archive)?.parsed(None)?.with_contracts(
        CatalogRecordSelection::All,
        None,
        |input| verify_contracts(&input, scratch.path()),
    )??;
    Ok(())
}

fn verify_contracts(
    input: &crate::catalog_queries::ContractSnapshot<'_>,
    scratch: &Path,
) -> TestResult {
    let mut texts = SelectedTexts::default();
    let staged = contracts::stage(input, scratch, None, &mut texts)?;
    let contract_table = serde_json::to_value(staged.rows)?;
    let title_table = serde_json::to_value(staged.titles)?;
    let contracts = array(&contract_table, "rows")?;
    let titles = array(&title_table, "rows")?;
    assert_eq!(
        contracts.len(),
        3,
        "all three canonical contract occurrences"
    );
    assert_eq!(
        contracts.len(),
        titles.len(),
        "each contract has one title occurrence"
    );
    for (contract, title) in contracts.iter().zip(titles) {
        assert_eq!(
            contract.get("source"),
            title.get("contract"),
            "the two projections share exact physical ownership"
        );
        assert_eq!(
            contract.pointer("/title/value"),
            title.pointer("/title/key/value"),
            "the contract refers to the same protected title key"
        );
        assert_eq!(
            contract.pointer("/canBeShared/value/value"),
            Some(&json!(true)),
            "sub contracts preserve the parent's chosen sharing value"
        );
        assert_eq!(
            contract.pointer("/canBeShared/layer/value"),
            Some(&json!("contract")),
            "the selected layer remains explicit"
        );
        assert_eq!(
            contract.pointer("/canBeShared/value/containsEncryptedSource"),
            Some(&json!(true)),
            "the original selected value retains its marker"
        );
    }
    let wire = serde_json::to_string(&contract_table)?;
    assert!(
        !wire.contains("sourceUsesRefs"),
        "detailed use lists stay out of this wire shape"
    );
    assert!(
        !wire.contains("sourceSetComplete"),
        "the immediate marker carries the selected classification"
    );
    Ok(())
}

fn array<'a>(data: &'a Value, name: &str) -> TestResult<&'a [Value]> {
    data.get(name)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| format!("expected array {name}").into())
}

fn check_text(
    row: &Value,
    field: &str,
    texts: &Value,
    english: &str,
    german: Option<&str>,
    original_key: &str,
) -> TestResult {
    let field = row.get(field).ok_or("selected projected text")?;
    let reference = field
        .pointer("/key/value")
        .ok_or("protected text reference")?;
    let definition = array(texts, "gameTexts")?
        .iter()
        .find(|definition| definition.pointer("/id/value") == Some(reference))
        .ok_or("selected key in the protected text table")?;
    let key = definition.pointer("/key/value").and_then(Value::as_str);
    assert_eq!(
        key.and_then(|key| key.strip_prefix('@')),
        Some(original_key)
    );
    assert_eq!(
        field
            .pointer("/key/containsEncryptedSource")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert!(field.get("itemTokens").is_none());
    for (language, expected) in [("en", Some(english)), ("de", german)] {
        let translated = field
            .pointer(&format!("/translations/{language}"))
            .ok_or("canonical per-contract translation")?;
        let source = definition
            .pointer(&format!("/translations/value/{language}/displayText"))
            .ok_or("protected translation closure")?;
        for result in [translated, source] {
            assert_eq!(result.get("value").and_then(Value::as_str), expected);
            assert_eq!(
                result.get("state").and_then(Value::as_str),
                Some(if expected.is_some() {
                    "present"
                } else {
                    "absent"
                })
            );
            assert!(result.get("sourceUses").is_none());
        }
    }
    Ok(())
}

fn session(archive: &Archive) -> TestResult<CatalogQuerySession<'_>> {
    Ok(CatalogQuerySession::open(
        archive,
        CatalogLanguageSelection::Selected(&[Language::English, Language::German]),
        None,
    )?)
}

fn archive(root: &Path) -> TestResult<Archive> {
    let path = root.join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", database()).encrypted())
        .entry(TestEntry::stored(
            Language::English.global_ini_path(),
            GlobalIniBuilder::new()
                .text("fixture_title", "Parent title")
                .text("fixture_description", "Parent description")
                .text("fixture_unused", "Unused")
                .build(),
        ))
        .entry(TestEntry::stored(
            Language::German.global_ini_path(),
            GlobalIniBuilder::new()
                .text("fixture_title", "Auftrag")
                .text("fixture_unused", "Unbenutzt")
                .build(),
        ))
        .write_to(&path)?;
    Ok(Archive::open_with_key(path, Some(test_key()))?)
}

fn database() -> Vec<u8> {
    DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new(GENERATOR)
                .array("generators", TestType::StrongPointer(HANDLER.to_owned())),
        )
        .structure(TestStruct::new(HANDLER).array("contracts", class(CONTRACT)))
        .structure(
            TestStruct::new(CONTRACT)
                .field("id", TestType::Guid)
                .field("debugName", TestType::String)
                .field("paramOverrides", class(PARAMS))
                .array("subContracts", class(CHILD)),
        )
        .structure(
            TestStruct::new(CHILD)
                .field("id", TestType::Guid)
                .field("debugName", TestType::String),
        )
        .structure(
            TestStruct::new(PARAMS)
                .array("stringParamOverrides", class(STRING_PARAM))
                .array("boolParamOverrides", class(BOOL_PARAM)),
        )
        .structure(
            TestStruct::new(BOOL_PARAM)
                .field("param", TestType::String)
                .field("value", TestType::Bool),
        )
        .structure(
            TestStruct::new(STRING_PARAM)
                .field("param", TestType::String)
                .field("value", TestType::Locale),
        )
        .record(generator(
            10,
            "A",
            contract(20).value(
                "subContracts",
                TestValue::Array(vec![TestValue::Struct(
                    TestInstance::new(CHILD)
                        .value("id", TestValue::Guid(30))
                        .value("debugName", TestValue::text("Child")),
                )]),
            ),
        ))
        .record(generator(11, "B", contract(21)))
        .build()
}

fn class(name: &str) -> TestType {
    TestType::Class(name.to_owned())
}

fn generator(id: u128, name: &str, contract: TestInstance) -> TestRecord {
    let handler = TestInstance::new(HANDLER).value(
        "contracts",
        TestValue::Array(vec![TestValue::Struct(contract)]),
    );
    TestRecord::new(
        id,
        GENERATOR,
        format!("ContractGenerator.{name}"),
        format!("libs/foundry/records/contracts/contractgenerator/{name}.xml"),
    )
    .value(
        "generators",
        TestValue::Array(vec![TestValue::Struct(handler)]),
    )
}

fn contract(id: u128) -> TestInstance {
    let parameters = [
        ("Title", "@fixture_title"),
        ("Description", "@fixture_description"),
    ]
    .into_iter()
    .map(|(name, key)| {
        TestValue::Struct(
            TestInstance::new(STRING_PARAM)
                .value("param", TestValue::text(name))
                .value("value", TestValue::text(key)),
        )
    })
    .collect();
    TestInstance::new(CONTRACT)
        .value("id", TestValue::Guid(id))
        .value("debugName", TestValue::text("Parent"))
        .value(
            "paramOverrides",
            TestValue::Struct(
                TestInstance::new(PARAMS)
                    .value("stringParamOverrides", TestValue::Array(parameters))
                    .value(
                        "boolParamOverrides",
                        TestValue::Array(vec![TestValue::Struct(
                            TestInstance::new(BOOL_PARAM)
                                .value("param", TestValue::text("CanBeShared"))
                                .value("value", TestValue::Bool(true)),
                        )]),
                    ),
            ),
        )
}
