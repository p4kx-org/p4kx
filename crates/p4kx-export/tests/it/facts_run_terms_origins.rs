// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Field, RecordTree, StructValue, Value};
use serde_json::{Value as Json, json};

use crate::catalog_contracts;
use crate::health_origin_assertions as origin;
use crate::health_origin_fixture::{self, Fixture, Input, TestResult};

use super::{GENERATOR, OVERRIDDEN, STRIPPED, SUB, database};

struct RunTerms {
    record: u32,
    results: u64,
    time: u64,
    buy_in: u64,
    stripped: u64,
}

#[test]
fn a_sub_contract_keeps_its_parent_run_terms_and_their_original_sources() -> TestResult {
    let data = database().build();
    for input in [Input::Plain, Input::Encrypted, Input::Standalone] {
        let fixture = Fixture::open(&data, input, Vec::new())?;
        let original = RunTerms::read(&fixture)?;
        let (_, catalog) = fixture.diagnostic()?;
        let parent = health_origin_fixture::guid(OVERRIDDEN);
        let child = health_origin_fixture::guid(SUB);
        for sub in [None, Some(child.as_str())] {
            let row = catalog_contracts::entry(&catalog, &parent, sub)?;
            original.check(&catalog, row, input.marker())?;
        }
        let stripped =
            catalog_contracts::entry(&catalog, &health_origin_fixture::guid(STRIPPED), None)?;
        original.check_absent(&catalog, stripped, input.marker())?;
        origin::closure(&catalog)?;
    }
    Ok(())
}

impl RunTerms {
    fn read(fixture: &Fixture) -> TestResult<Self> {
        let forge = fixture.entry.open()?;
        let record = forge.record(3).ok_or("original Generator Record")?;
        assert_eq!(
            record.guid().to_string(),
            health_origin_fixture::guid(GENERATOR),
            "the selected physical record is the fixture Generator"
        );
        assert_eq!(
            record.type_name()?,
            "ContractGenerator",
            "the original Generator source type"
        );
        let tree = record.tree()?;
        assert!(tree.is_from(&record), "actual original Record ownership");
        assert_eq!(tree.error_count(), 0, "all original run terms are readable");

        let handler = element(tree.root(), "generators", 0)?;
        let bare_handler = element(tree.root(), "generators", 1)?;
        let parent = element(structure(handler)?, "contracts", 1)?;
        let stripped = element(structure(bare_handler)?, "contracts", 0)?;
        let parent = structure(parent)?;
        let child = structure(element(parent, "subContracts", 0)?)?;
        check_id(parent, OVERRIDDEN)?;
        check_id(child, SUB)?;
        let bare = structure(stripped)?;
        check_id(bare, STRIPPED)?;
        assert!(
            bare.field("contractResults").is_none(),
            "no authored results field"
        );

        let results = field(parent, "contractResults")?;
        let values = structure(results)?;
        let time = field(values, "timeToComplete")?;
        let buy_in = field(values, "contractBuyInAmount")?;
        assert_eq!(
            time,
            &Value::Single(25.0),
            "the parent stores a Single duration"
        );
        assert_eq!(
            buy_in,
            &Value::Int32(1500),
            "the parent stores an Int32 buy-in"
        );
        Ok(Self {
            record: record.index(),
            results: ordinal(&tree, results)?,
            time: ordinal(&tree, time)?,
            buy_in: ordinal(&tree, buy_in)?,
            stripped: ordinal(&tree, stripped)?,
        })
    }

    fn check(&self, catalog: &Json, row: &Json, marker: Option<bool>) -> TestResult {
        for (field, expected, ordinal) in [
            ("timeToComplete", json!(25.0), self.time),
            ("buyInAmount", json!(1500), self.buy_in),
        ] {
            let cell = origin::present(row, field, &expected, marker)?;
            let uses = origin::uses(catalog, cell)?;
            assert!(
                uses.iter()
                    .any(|used| { self.source_use(used, ordinal, "$value", "payload") }),
                "{field} uses the parent's original primitive"
            );
            assert!(
                uses.iter()
                    .any(|used| { self.source_use(used, self.results, "$value", "selector") }),
                "{field} retains the original selected results block"
            );
        }
        Ok(())
    }

    fn check_absent(&self, catalog: &Json, row: &Json, marker: Option<bool>) -> TestResult {
        for field in ["timeToComplete", "buyInAmount"] {
            let cell = origin::cell(row, field, "absent")?;
            assert_eq!(
                cell.get("containsEncryptedSource"),
                Some(&json!(marker)),
                "{field} retains the original declaration source marker"
            );
            assert_eq!(
                cell.get("sourceSetComplete"),
                Some(&json!(true)),
                "{field} absence is completely observed"
            );
            assert!(
                origin::uses(catalog, cell)?
                    .iter()
                    .any(|used| { self.source_use(used, self.stripped, "$fields", "selector") }),
                "{field} absence is supported by the actual declaration survey"
            );
        }
        Ok(())
    }

    fn source_use(&self, used: &Json, ordinal: u64, field: &str, role: &str) -> bool {
        used.pointer("/location/namespace").and_then(Json::as_str) == Some("dataforgeValue")
            && used.pointer("/location/indices")
                == Some(&json!([self.record.to_string(), ordinal.to_string()]))
            && used.pointer("/location/field").and_then(Json::as_str) == Some(field)
            && used.get("role").and_then(Json::as_str) == Some(role)
    }
}

fn field<'tree, 'source>(
    parent: &'tree StructValue<'source>,
    name: &str,
) -> TestResult<&'tree Value<'source>> {
    parent
        .field(name)
        .ok_or_else(|| format!("original field {name}").into())
}

fn element<'tree, 'source>(
    parent: &'tree StructValue<'source>,
    name: &str,
    index: usize,
) -> TestResult<&'tree Value<'source>> {
    let Value::Array(values) = field(parent, name)? else {
        return Err("expected an original array".into());
    };
    values
        .get(index)
        .ok_or_else(|| "original array slot".into())
}

fn structure<'tree, 'source>(
    value: &'tree Value<'source>,
) -> TestResult<&'tree StructValue<'source>> {
    let Value::Struct(value) = value else {
        return Err("expected an original struct".into());
    };
    Ok(value)
}

fn check_id(value: &StructValue<'_>, expected: u128) -> TestResult {
    let Value::Guid(id) = field(value, "id")? else {
        return Err("expected the authored contract id".into());
    };
    assert_eq!(
        id.to_string(),
        health_origin_fixture::guid(expected),
        "the source occurrence has the expected authored contract ID"
    );
    Ok(())
}

fn ordinal<'source>(tree: &RecordTree<'source>, selected: &Value<'source>) -> TestResult<u64> {
    let mut pending: Vec<_> = tree.fields().iter().rev().map(Field::value).collect();
    let mut ordinal = 0_u64;
    while let Some(value) = pending.pop() {
        if std::ptr::eq(value, selected) {
            return Ok(ordinal);
        }
        ordinal = ordinal.checked_add(1).ok_or("source ordinal overflow")?;
        match value {
            Value::Struct(owner) => pending.extend(owner.fields().iter().rev().map(Field::value)),
            Value::Array(values) => pending.extend(values.iter().rev()),
            _ => {}
        }
    }
    Err("selected value has no original occurrence".into())
}

#[path = "facts_run_terms_numeric.rs"]
mod numeric;
