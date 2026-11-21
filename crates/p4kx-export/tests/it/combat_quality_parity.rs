// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value, json};

use super::combat_quality_part;
use super::combat_quality_ship;
use super::combat_quality_vectors::{Case, Fixture, Stat, Step, Tolerance};

pub(super) type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const VECTORS: &str = include_str!("../../../../docs/examples/catalog-combat-quality-v1.json");

#[test]
fn shared_combat_quality_vectors_match_formula_version_one() -> TestResult {
    let fixture: Fixture = serde_json::from_str(VECTORS)?;
    assert_eq!(
        (
            fixture.version,
            fixture.formula_version,
            fixture.catalog_schema_version,
            fixture.default_quality
        ),
        (1, 1, 3, 500),
        "shared fixture, formula and Catalog versions",
    );
    let units = &fixture.units;
    assert_eq!(
        [
            units.damage.as_str(),
            units.nominal_dps.as_str(),
            units.travel_distance.as_str(),
            units.shield.as_str(),
            units.rate.as_str()
        ],
        [
            "damagePoints",
            "damagePointsPerSecond",
            "metres",
            "hitPoints",
            "dischargesPerMinute"
        ],
        "units on the reproducible test vector inputs",
    );
    assert!(
        fixture.tolerance.absolute > 0.0 && fixture.tolerance.relative > 0.0,
        "declared fixture tolerances"
    );
    assert!(
        fixture.property_mappings.iter().any(|mapping| matches!(
            mapping.better_when,
            super::combat_quality_vectors::Direction::Lower
        )),
        "the fixture exercises independent presentation direction"
    );
    let mapping = mapping(&fixture);
    let mut cases = BTreeSet::new();
    for case in &fixture.cases {
        assert!(cases.insert(&case.id), "one named fixture case");
        inspect_source(case);
        let actual = expected(case, fixture.default_quality, &mapping)?;
        compare(&actual, &case.expected, &fixture.tolerance, &case.id)?;
    }
    Ok(())
}

fn mapping(fixture: &Fixture) -> BTreeMap<&str, Stat> {
    fixture
        .property_mappings
        .iter()
        .map(|mapping| (mapping.property_id.as_str(), mapping.target))
        .collect()
}

fn inspect_source(case: &Case) {
    let build = &case.build;
    assert!(
        !build.branch.is_empty()
            && !build.version.is_empty()
            && !build.change_number.is_empty()
            && !build.id.is_empty(),
        "source Build identity in {}",
        case.id,
    );
    let mut parts = BTreeSet::new();
    for part in &case.parts {
        assert!(
            parts.insert(&part.id),
            "one fixture item definition per part ID"
        );
        for guid in [&part.item_guid, &part.blueprint_guid] {
            assert!(
                guid.len() == 36,
                "synthetic physical selection uses GUID format"
            );
        }
        let mut paths = BTreeSet::new();
        for slot in &part.slots {
            let parent = matches!(
                slot.slot_path.first(),
                Some(Step::Mandatory | Step::Optional { .. })
            );
            let offsets = slot
                .slot_path
                .iter()
                .skip(1)
                .all(|part| matches!(part, Step::Option { .. }));
            assert!(
                parent && offsets && paths.insert(&slot.slot_path),
                "strict, unique source position through active branches"
            );
        }
    }
}

fn expected(case: &Case, default: u32, mapping: &BTreeMap<&str, Stat>) -> TestResult<Value> {
    let mut parts = Map::new();
    for part in &case.parts {
        parts.insert(part.id.clone(), json!({
            "factory": combat_quality_part::calculate(part, None, default, mapping),
            "crafted": combat_quality_part::calculate(part, Some(&part.qualities), default, mapping),
        }));
    }
    let mut whole = Map::new();
    whole.insert("parts".into(), Value::Object(parts));
    if let Some(loadouts) = &case.loadouts {
        whole.insert("ship".into(), json!({
            "factory": combat_quality_ship::calculate(case, &loadouts.factory, false, default, mapping)?,
            "crafted": combat_quality_ship::calculate(case, &loadouts.crafted, true, default, mapping)?,
        }));
    }
    Ok(Value::Object(whole))
}

fn compare(actual: &Value, expected: &Value, tolerance: &Tolerance, path: &str) -> TestResult {
    match (actual, expected) {
        (Value::Object(actual), Value::Object(expected)) => {
            assert_eq!(
                actual.len(),
                expected.len(),
                "output field coverage at {path}"
            );
            for (field, expected) in expected {
                let actual = actual
                    .get(field)
                    .ok_or_else(|| format!("missing {path}/{field}"))?;
                compare(actual, expected, tolerance, &format!("{path}/{field}"))?;
            }
        }
        (Value::Number(actual), Value::Number(expected)) if actual.is_f64() => {
            let actual = actual.as_f64().ok_or("numeric result")?;
            let expected = expected.as_f64().ok_or("numeric fixture expectation")?;
            let maximum = tolerance.absolute.max(tolerance.relative * expected.abs());
            assert!(
                (actual - expected).abs() <= maximum,
                "{path}: calculated {actual}, expected {expected}, tolerance {maximum}"
            );
        }
        _ => assert_eq!(actual, expected, "exact output at {path}"),
    }
    Ok(())
}
