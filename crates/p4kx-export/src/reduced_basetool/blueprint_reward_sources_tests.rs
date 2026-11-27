// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::path::Path;

use p4kx_extraction::{Archive, Language};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, GlobalIniBuilder, TestEntry, TestInstance, TestRecord,
    TestStruct, TestType, TestValue, test_key,
};
use serde_json::Value;

use super::super::record_targets::RecordTargets;
use super::super::selected_texts::SelectedTexts;
use super::{ContractPosition, PATH, ReducedError, collect};
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection,
};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const POOL: &str = "BlueprintPoolRecord";
const GENERATOR: &str = "ContractGenerator";
const HANDLER: &str = "ContractGeneratorHandler_List";
const CONTRACT: &str = "CareerContract";
const PARAMS: &str = "ContractParamOverrides";
const PARAM: &str = "ContractStringParam";
const REWARD: &str = "Reward";
const NESTED_REWARD: &str = "NestedReward";
const OUTSIDE: &str = "OtherDefinition";
const POOL_GUID: u128 = 10;
const CONTRACT_ID: u128 = 20;

#[test]
fn reward_source_links_only_to_the_exact_published_contract_ancestor() -> TestResult {
    let scratch = tempfile::tempdir()?;
    let archive = archive(scratch.path())?;
    let session = CatalogQuerySession::open(
        &archive,
        CatalogLanguageSelection::Selected(&[Language::English, Language::German]),
        None,
    )?;
    let parsed = session.parsed(None)?;
    let pools = parsed.with_blueprint_pools(
        CatalogRecordSelection::All,
        None,
        |snapshot| -> Result<_, ReducedError> {
            let mut pools = RecordTargets::default();
            for pool in snapshot.rows() {
                pools.insert(PATH, pool.source())?;
            }
            Ok(pools)
        },
    )??;
    parsed.with_contracts(
        CatalogRecordSelection::All,
        None,
        |snapshot| -> TestResult {
            let mut texts = SelectedTexts::default();
            let table = collect(&parsed, &snapshot, &pools, scratch.path(), None, &mut texts)?;
            let [contract] = snapshot.rows.as_slice() else {
                return Err("one published contract after canonical deduplication".into());
            };
            let source = serde_json::to_value(ContractPosition::from(&contract.base.source))?;
            assert_projection(
                &serde_json::to_value(table)?,
                &serde_json::to_value(texts)?,
                &source,
            )?;
            let mut texts = SelectedTexts::default();
            assert!(matches!(
                collect(
                    &parsed,
                    &snapshot,
                    &RecordTargets::default(),
                    scratch.path(),
                    None,
                    &mut texts
                ),
                Err(ReducedError::UnresolvedRecordLink {
                    path: PATH,
                    index: 0
                })
            ));
            Ok(())
        },
    )??;
    Ok(())
}

fn assert_projection(table: &Value, texts: &Value, source: &Value) -> TestResult {
    let rows = table
        .get("rows")
        .and_then(Value::as_array)
        .ok_or("all canonical rewards")?;
    let [root, winner, nested, duplicate, outside] = rows.as_slice() else {
        return Err("root, published, nested, duplicate, and non-Generator declarations".into());
    };
    let mut last_ordinal = None;
    for (row, status, chance, index) in [
        (root, "noAuthoredId", 0.125, 1),
        (winner, "resolved", 0.25, 1),
        (nested, "unpublished", 0.375, 1),
        (duplicate, "unpublished", 0.5, 1),
        (outside, "outsideGenerator", 0.75, 2),
    ] {
        assert_eq!(
            row.get("contractStatus").and_then(Value::as_str),
            Some(status)
        );
        assert_eq!(
            row.pointer("/pool/value/targetRecordIndex/value"),
            Some(&Value::from(0))
        );
        assert_eq!(row.pointer("/recordIndex/value"), Some(&Value::from(index)));
        assert_eq!(
            row.pointer("/chance/value/finite").and_then(Value::as_f64),
            Some(chance)
        );
        assert_eq!(
            row.pointer("/chance/containsEncryptedSource"),
            Some(&Value::Bool(true))
        );
        if index == 1 {
            let ordinal: u64 = row
                .pointer("/sourceOrdinal/value")
                .and_then(Value::as_str)
                .ok_or("source ordinal")?
                .parse()?;
            assert!(last_ordinal.is_none_or(|previous| ordinal > previous));
            last_ordinal = Some(ordinal);
        }
        if status == "resolved" {
            assert_eq!(row.get("contract"), Some(source));
        } else {
            assert_eq!(row.get("contract"), Some(&Value::Null));
            assert_eq!(row.get("title"), Some(&Value::Null));
        }
    }
    assert_title_selection(root, winner, &[nested, duplicate, outside], texts)
}

fn assert_title_selection(
    root: &Value,
    winner: &Value,
    unresolved: &[&Value],
    texts: &Value,
) -> TestResult {
    for row in unresolved {
        assert_eq!(
            row.pointer("/contractId/value"),
            winner.pointer("/contractId/value"),
            "authored GUID alone does not select a title"
        );
    }
    assert_eq!(
        root.pointer("/contractId/state").and_then(Value::as_str),
        Some("absent")
    );
    let definitions = texts
        .get("gameTexts")
        .and_then(Value::as_array)
        .ok_or("protected text table")?;
    let [title] = definitions.as_slice() else {
        return Err("exactly one published reward title".into());
    };
    assert_eq!(title.pointer("/id/value"), winner.pointer("/title/value"));
    assert_eq!(
        title
            .pointer("/translations/value/en/displayText/value")
            .and_then(Value::as_str),
        Some("Published")
    );
    assert_eq!(
        title
            .pointer("/translations/value/de/displayText/value")
            .and_then(Value::as_str),
        Some("Veröffentlicht")
    );
    Ok(())
}

fn archive(root: &Path) -> TestResult<Archive> {
    let path = root.join("Data.p4k");
    let archive = ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", database()).encrypted())
        .entry(localization(Language::English, "Published"))
        .entry(localization(Language::German, "Veröffentlicht"));
    archive.write_to(&path)?;
    Ok(Archive::open_with_key(path, Some(test_key()))?)
}

fn localization(language: Language, winner: &str) -> TestEntry {
    TestEntry::stored(
        language.global_ini_path(),
        GlobalIniBuilder::new()
            .text("fixture_winner", winner)
            .text("fixture_duplicate", "Losing duplicate")
            .build(),
    )
}

fn database() -> Vec<u8> {
    let builder = DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new(POOL))
        .structure(
            TestStruct::new(GENERATOR)
                .field("globalPool", TestType::Reference)
                .field("chance", TestType::Single)
                .array("generators", pointer(HANDLER)),
        )
        .structure(TestStruct::new(HANDLER).array("contracts", class(CONTRACT)))
        .structure(
            TestStruct::new(CONTRACT)
                .field("id", TestType::Guid)
                .field("debugName", TestType::String)
                .field("paramOverrides", class(PARAMS))
                .array("authoredRewards", pointer(REWARD)),
        )
        .structure(TestStruct::new(PARAMS).array("stringParamOverrides", class(PARAM)))
        .structure(
            TestStruct::new(PARAM)
                .field("param", TestType::String)
                .field("value", TestType::Locale),
        )
        .structure(pool_fields(TestStruct::new(REWARD)))
        .structure(
            TestStruct::new(NESTED_REWARD)
                .parent(REWARD)
                .field("id", TestType::Guid),
        )
        .structure(pool_fields(
            TestStruct::new(OUTSIDE).field("id", TestType::Guid),
        ));
    builder
        .record(TestRecord::new(
            POOL_GUID,
            POOL,
            "BlueprintPoolRecord.Primary",
            "libs/foundry/records/crafting/blueprintrewards/primary.xml",
        ))
        .record(generator())
        .record(
            TestRecord::new(
                30,
                OUTSIDE,
                "OtherDefinition.Outside",
                "fixture/outside.xml",
            )
            .value("id", TestValue::Guid(CONTRACT_ID))
            .value("pool", TestValue::Reference(POOL_GUID))
            .value("chance", TestValue::Single(0.75)),
        )
        .build()
}

fn generator() -> TestRecord {
    let winner = contract(
        "fixture_winner",
        vec![
            reward(REWARD, 0.25),
            reward(NESTED_REWARD, 0.375).value("id", TestValue::Guid(CONTRACT_ID)),
        ],
    );
    let duplicate = contract("fixture_duplicate", vec![reward(REWARD, 0.5)]);
    let handler = TestInstance::new(HANDLER).value(
        "contracts",
        TestValue::Array(vec![
            TestValue::Struct(winner),
            TestValue::Struct(duplicate),
        ]),
    );
    TestRecord::new(
        40,
        GENERATOR,
        "ContractGenerator.Shared",
        "libs/foundry/records/contracts/contractgenerator/shared.xml",
    )
    .value("globalPool", TestValue::Reference(POOL_GUID))
    .value("chance", TestValue::Single(0.125))
    .value(
        "generators",
        TestValue::Array(vec![TestValue::Struct(handler)]),
    )
}

fn contract(key: &str, rewards: Vec<TestInstance>) -> TestInstance {
    let title = TestInstance::new(PARAM)
        .value("param", TestValue::text("Title"))
        .value("value", TestValue::text(format!("@{key}")));
    TestInstance::new(CONTRACT)
        .value("id", TestValue::Guid(CONTRACT_ID))
        .value("debugName", TestValue::text(key))
        .value(
            "paramOverrides",
            TestValue::Struct(TestInstance::new(PARAMS).value(
                "stringParamOverrides",
                TestValue::Array(vec![TestValue::Struct(title)]),
            )),
        )
        .value(
            "authoredRewards",
            TestValue::Array(rewards.into_iter().map(TestValue::Struct).collect()),
        )
}

fn reward(kind: &str, chance: f32) -> TestInstance {
    TestInstance::new(kind)
        .value("pool", TestValue::Reference(POOL_GUID))
        .value("chance", TestValue::Single(chance))
}

fn pool_fields(value: TestStruct) -> TestStruct {
    value
        .field("pool", TestType::Reference)
        .field("chance", TestType::Single)
}

fn pointer(kind: &str) -> TestType {
    TestType::StrongPointer(kind.to_owned())
}

fn class(kind: &str) -> TestType {
    TestType::Class(kind.to_owned())
}
