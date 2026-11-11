// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{RecordTree, Value, ValueError};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};
use serde_json::{Value as Json, json};

use crate::catalog_contracts;
use crate::health_origin_assertions as origin;
use crate::health_origin_fixture::{Fixture, Input, TestResult, guid};
use crate::health_reference_origins;

use super::{check_id, element, field, ordinal, structure};

const GENERATOR: u128 = 0x74b0;
const ROUNDED: u128 = 0x74b1;
const OVERFLOW: u128 = 0x74b2;
const UNREADABLE: u128 = 0x74b3;
const PRECISE_TIME: f64 = 16_777_217.0;
const SIGNED_BOUND: u64 = i64::MAX.cast_unsigned();

const WIDE_HANDLER: &str = "RunTermsWideHandler";
const WIDE_CONTRACT: &str = "RunTermsWideContract";
const WIDE_RESULTS: &str = "RunTermsWideResults";
const ERROR_HANDLER: &str = "RunTermsErrorHandler";
const ERROR_CONTRACT: &str = "RunTermsErrorContract";
const ERROR_RESULTS: &str = "RunTermsErrorResults";

#[test]
fn run_terms_keep_numeric_projection_and_isolate_an_unreadable_sibling() -> TestResult {
    let fixture = Fixture::open(&database(), Input::Plain, Vec::new())?;
    let [rounded, overflow, unreadable] = original(&fixture)?;
    let (_, catalog) = fixture.diagnostic()?;
    let arena = arena(&catalog)?;
    rounded.check_rounded(&catalog, arena)?;
    overflow.check_overflow(&catalog, arena)?;
    unreadable.check_unreadable(&catalog, arena)?;
    origin::closure(&catalog)
}

fn database() -> Vec<u8> {
    let builder = schema();
    let wide = TestInstance::new(WIDE_HANDLER).value(
        "contracts",
        TestValue::Array(vec![
            contract(
                (WIDE_CONTRACT, WIDE_RESULTS),
                ROUNDED,
                TestValue::Double(PRECISE_TIME),
                SIGNED_BOUND,
            ),
            contract(
                (WIDE_CONTRACT, WIDE_RESULTS),
                OVERFLOW,
                TestValue::Double(f64::MAX),
                u64::MAX,
            ),
        ]),
    );
    let broken = TestInstance::new(ERROR_HANDLER).value(
        "contracts",
        TestValue::Array(vec![contract(
            (ERROR_CONTRACT, ERROR_RESULTS),
            UNREADABLE,
            TestValue::Raw(vec![3]),
            1500,
        )]),
    );
    builder
        .record(
            TestRecord::new(
                GENERATOR,
                "ContractGenerator",
                "ContractGenerator.RunTermsNumeric",
                "libs/foundry/records/contracts/contractgenerator/run_terms_numeric.xml",
            )
            .value(
                "generators",
                TestValue::Array(vec![TestValue::Struct(wide), TestValue::Struct(broken)]),
            ),
        )
        .build()
}

fn schema() -> DataForgeBuilder {
    let mut builder = DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("ContractGenerator").array(
            "generators",
            TestType::StrongPointer("ContractHandler".to_owned()),
        ))
        .structure(TestStruct::new("ContractHandler"));
    for (handler, contract, results, time) in [
        (WIDE_HANDLER, WIDE_CONTRACT, WIDE_RESULTS, TestType::Double),
        (ERROR_HANDLER, ERROR_CONTRACT, ERROR_RESULTS, TestType::Bool),
    ] {
        builder = builder
            .structure(
                TestStruct::new(handler)
                    .parent("ContractHandler")
                    .array("contracts", TestType::Class(contract.to_owned())),
            )
            .structure(
                TestStruct::new(contract)
                    .field("id", TestType::Guid)
                    .field("contractResults", TestType::Class(results.to_owned())),
            )
            .structure(
                TestStruct::new(results)
                    .field("timeToComplete", time)
                    .field("contractBuyInAmount", TestType::UInt64),
            );
    }
    builder
}

fn contract(types: (&str, &str), id: u128, time: TestValue, buy_in: u64) -> TestValue {
    TestValue::Struct(
        TestInstance::new(types.0)
            .value("id", TestValue::Guid(id))
            .value(
                "contractResults",
                TestValue::Struct(
                    TestInstance::new(types.1)
                        .value("timeToComplete", time)
                        .value("contractBuyInAmount", TestValue::UInt64(buy_in)),
                ),
            ),
    )
}

struct Occurrence {
    record: u32,
    results: u64,
    time: u64,
    buy_in: u64,
}

fn original(fixture: &Fixture) -> TestResult<[Occurrence; 3]> {
    let forge = fixture.entry.open()?;
    let record = forge
        .record(0)
        .ok_or("the original numeric Generator Record")?;
    assert_eq!(
        record.guid().to_string(),
        guid(GENERATOR),
        "actual Generator identity"
    );
    assert_eq!(
        record.type_name()?,
        "ContractGenerator",
        "actual Generator type"
    );
    let tree = record.tree()?;
    assert!(
        tree.is_from(&record),
        "the numeric source tree has its genuine original owner"
    );
    assert_eq!(
        tree.error_count(),
        1,
        "only the deliberate malformed time field fails to read"
    );
    Ok([
        occurrence(
            &tree,
            (0, 0, ROUNDED),
            (&Value::Double(PRECISE_TIME), SIGNED_BOUND),
        )?,
        occurrence(
            &tree,
            (0, 1, OVERFLOW),
            (&Value::Double(f64::MAX), u64::MAX),
        )?,
        occurrence(
            &tree,
            (1, 0, UNREADABLE),
            (&Value::Error(ValueError::InvalidBool { byte: 3 }), 1500),
        )?,
    ])
}

fn occurrence(
    tree: &RecordTree<'_>,
    (handler, slot, guid): (usize, usize, u128),
    (time, buy_in): (&Value<'_>, u64),
) -> TestResult<Occurrence> {
    let handler = structure(element(tree.root(), "generators", handler)?)?;
    let contract = structure(element(handler, "contracts", slot)?)?;
    check_id(contract, guid)?;
    let results = field(contract, "contractResults")?;
    let source = structure(results)?;
    let actual_time = field(source, "timeToComplete")?;
    let actual_buy_in = field(source, "contractBuyInAmount")?;
    assert_eq!(
        actual_time, time,
        "exact original time value or decode failure"
    );
    assert_eq!(
        actual_buy_in,
        &Value::UInt64(buy_in),
        "exact original UInt64 buy-in"
    );
    Ok(Occurrence {
        record: tree.record_index(),
        results: ordinal(tree, results)?,
        time: ordinal(tree, actual_time)?,
        buy_in: ordinal(tree, actual_buy_in)?,
    })
}

impl Occurrence {
    fn check_rounded(&self, catalog: &Json, arena: &[Json]) -> TestResult {
        let row = catalog_contracts::entry(catalog, &guid(ROUNDED), None)?;
        let time = origin::present(row, "timeToComplete", &json!(16_777_216.0), Some(false))?;
        let buy_in = origin::present(row, "buyInAmount", &json!(i64::MAX), Some(false))?;
        self.exact(catalog, time, Some((self.time, "conversion")))?;
        self.exact(catalog, buy_in, Some((self.buy_in, "payload")))?;
        self.check_raw_numbers(arena, PRECISE_TIME, SIGNED_BOUND)
    }

    fn check_overflow(&self, catalog: &Json, arena: &[Json]) -> TestResult {
        let row = catalog_contracts::entry(catalog, &guid(OVERFLOW), None)?;
        for (name, ordinal) in [("timeToComplete", self.time), ("buyInAmount", self.buy_in)] {
            let cell = origin::cell(row, name, "absent")?;
            assert_eq!(
                cell.get("sourceSetComplete"),
                Some(&json!(true)),
                "{name} overflow was read completely"
            );
            assert_eq!(
                cell.get("containsEncryptedSource"),
                Some(&json!(false)),
                "{name} overflow retains its readable plain source"
            );
            self.exact(catalog, cell, Some((ordinal, "selector")))?;
        }
        self.check_raw_numbers(arena, f64::MAX, u64::MAX)
    }

    fn check_unreadable(&self, catalog: &Json, arena: &[Json]) -> TestResult {
        let row = catalog_contracts::entry(catalog, &guid(UNREADABLE), None)?;
        let time = origin::cell(row, "timeToComplete", "unreadable")?;
        assert_eq!(
            time.get("sourceSetComplete"),
            Some(&json!(false)),
            "the actual malformed time cannot certify a complete source set"
        );
        assert_eq!(
            time.get("containsEncryptedSource"),
            Some(&Json::Null),
            "an incomplete plain source set has unknown encryption"
        );
        self.exact(catalog, time, None)?;
        let inventory = origin::rows(catalog, "/provenance/uses")?;
        assert!(
            inventory.iter().any(|used| {
                used.pointer("/location/namespace") == Some(&json!("dataforgeValue"))
                    && used.pointer("/location/indices")
                        == Some(&json!([self.record.to_string(), self.time.to_string()]))
                    && used.pointer("/location/field") == Some(&json!("$value"))
                    && used.get("role") == Some(&json!("attempted"))
            }),
            "the malformed original time remains an attempted use outside both cells"
        );
        let buy_in = origin::present(row, "buyInAmount", &json!(1500), Some(false))?;
        self.exact(catalog, buy_in, Some((self.buy_in, "payload")))?;
        let raw = node(arena, self.time)?;
        assert_eq!(
            origin::at(raw, "/originalError/type")?,
            "invalidBool",
            "the definition retains the exact typed original failure"
        );
        assert_eq!(
            origin::at(raw, "/originalError/byte/value")?,
            &json!(3),
            "the definition retains the original malformed byte"
        );
        Ok(())
    }

    fn check_raw_numbers(&self, arena: &[Json], time: f64, buy_in: u64) -> TestResult {
        let duration = node(arena, self.time)?;
        let amount = node(arena, self.buy_in)?;
        assert_eq!(
            origin::at(duration, "/value/value/value/type")?,
            "f64",
            "the original duration remains a Double"
        );
        assert_eq!(
            origin::at(duration, "/value/value/value/rawBits")?,
            &json!(format!("{:016x}", time.to_bits())),
            "the definition retains the complete original duration bits"
        );
        assert_eq!(
            origin::at(amount, "/value/value/value")?,
            &json!({"type":"uint64","value":buy_in.to_string()}),
            "the definition retains the original unsigned width and digits"
        );
        Ok(())
    }

    fn exact(&self, catalog: &Json, cell: &Json, datum: Option<(u64, &str)>) -> TestResult {
        let coordinate = |ordinal: u64, role: &str| {
            (
                "dataforgeValue".to_owned(),
                vec![self.record.to_string(), ordinal.to_string()],
                "$value".to_owned(),
                role.to_owned(),
            )
        };
        let mut expected = vec![coordinate(self.results, "selector")];
        if let Some((ordinal, role)) = datum {
            expected.push(coordinate(ordinal, role));
        }
        health_reference_origins::exact(catalog, cell, &expected)
    }
}

fn arena(catalog: &Json) -> TestResult<&[Json]> {
    let [definition] = origin::rows(catalog, "/definitions/contractGenerators/rows")? else {
        return Err("exactly one physical numeric Generator definition".into());
    };
    assert_eq!(
        origin::at(definition, "/guid/value")?,
        &json!(guid(GENERATOR)),
        "the definition belongs to the actual numeric Generator"
    );
    assert_eq!(
        origin::at(definition, "/recordIndex/value")?,
        &json!(0),
        "the original physical definition position"
    );
    origin::rows(definition, "/source/values")
}

fn node(arena: &[Json], ordinal: u64) -> TestResult<&Json> {
    arena
        .get(usize::try_from(ordinal)?)
        .ok_or_else(|| "exact original source occurrence in the retained arena".into())
}
