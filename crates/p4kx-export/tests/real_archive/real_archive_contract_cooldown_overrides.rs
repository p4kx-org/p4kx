// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The cooldown fields, their levels and `cooldownLayer` of contracts in the Basetool catalog of a
//! real Archive, checked against the parameter overrides in the `ContractGenerator` Records; runs
//! with `--ignored` when `P4KX_DATA_P4K` names one.

use std::collections::{BTreeMap, HashMap};
use std::error::Error;
use std::time::SystemTime;

use p4kx_export::{CatalogOptions, build_catalog};
use p4kx_extraction::{Archive, DataForgeEntry, StructValue, TreeBuilder, Value};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::Value as Json;

use crate::real_archive_contracts::v3;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

/// `(record GUID, contract GUID, sub contract GUID or empty)`.
type EntryKey = (String, String, String);

/// The five stated fields of every entry.
type Expected = HashMap<EntryKey, [Stated; 5]>;

/// The overrides of one level by lower-case parameter name.
type Overrides = HashMap<String, Json>;

/// The level and stored value of one field, `None` when no level states it.
type Stated = Option<(&'static str, Json)>;

/// `(catalog field, override parameter)`; the field of the handler's `defaultAvailability` is the
/// catalog field.
const FIELDS: [(&str, &str); 5] = [
    ("hasPersonalCooldown", "HasPersonalCooldown"),
    ("personalCooldownTime", "PersonalCooldownTime"),
    (
        "personalCooldownTimeVariation",
        "PersonalCooldownTimeVariation",
    ),
    ("abandonedCooldownTime", "AbandonedCooldownTime"),
    (
        "abandonedCooldownTimeVariation",
        "AbandonedCooldownTimeVariation",
    ),
];
const CONTAINERS: [&str; 4] = [
    "contracts",
    "introContracts",
    "legacyContracts",
    "PVPBountyContract",
];
/// Nearest first, the order of `cooldownLayer`.
const LEVELS: [&str; 3] = ["subContract", "contract", "handler"];

#[expect(
    clippy::print_stdout,
    reason = "the figures are the result of the verification"
)]
fn print_line(line: &str) {
    println!("{line}");
}

/// The structs the field `field` of `parent` holds, as an array or as a single struct.
fn structs<'a>(
    parent: &'a StructValue<'_>,
    field: &str,
) -> impl Iterator<Item = &'a StructValue<'a>> {
    let items: &[Value<'_>] = match parent.field(field) {
        Some(Value::Array(items)) => items,
        Some(single @ Value::Struct(_)) => std::slice::from_ref(single),
        _ => &[],
    };
    items.iter().filter_map(|item| match item {
        Value::Struct(inner) => Some(inner),
        _ => None,
    })
}

/// The stored number or flag of `value`.
fn stored(value: Option<&Value<'_>>) -> Option<Json> {
    match value? {
        Value::Bool(flag) => Some(Json::Bool(*flag)),
        Value::Int32(number) => Some(Json::from(f64::from(*number))),
        Value::Single(number) => Some(Json::from(f64::from(*number))),
        _ => None,
    }
}

/// The bool and integer overrides of `params`.
fn overrides(params: &StructValue<'_>) -> Overrides {
    ["boolParamOverrides", "intParamOverrides"]
        .iter()
        .flat_map(|array| structs(params, array))
        .filter_map(|param| {
            let Some(Value::Enum(name) | Value::String(name)) = param.field("param") else {
                return None;
            };
            Some((name.to_ascii_lowercase(), stored(param.field("value"))?))
        })
        .collect()
}

/// The five fields as the nearest of `levels` (sub contract, contract, handler overrides) states
/// them, else the handler's `defaultAvailability`.
fn resolve(levels: [Option<&Overrides>; 3], availability: Option<&StructValue<'_>>) -> [Stated; 5] {
    FIELDS.map(|(field, param)| {
        let key = param.to_ascii_lowercase();
        LEVELS
            .iter()
            .zip(levels)
            .find_map(|(level, map)| Some((*level, map?.get(&key)?.clone())))
            .or_else(|| Some(("handler", stored(availability?.field(field))?)))
    })
}

/// Whether `overrides` states one of the five fields.
fn states_cooldown(overrides: &Overrides) -> bool {
    FIELDS
        .iter()
        .any(|(_, param)| overrides.contains_key(&param.to_ascii_lowercase()))
}

/// The expected fields of every entry of the Records, and how many entries a handler's
/// `contractParams` overrides a cooldown field of.
fn expected_fields(archive: &Archive) -> Result<(Expected, usize), Box<dyn Error>> {
    let entry = DataForgeEntry::read(archive)?;
    let forge = entry.open()?;
    let mut trees = TreeBuilder::new();
    let mut expected = HashMap::new();
    let mut from_handler_params = 0_usize;
    for record in forge.records() {
        if !record
            .record_path()?
            .contains("contracts/contractgenerator")
            || record.type_name()? != "ContractGenerator"
        {
            continue;
        }
        let tree = trees.build(&record)?;
        let record_guid = tree.guid().to_string();
        for handler in structs(tree.root(), "generators") {
            let handler_params = structs(handler, "contractParams").next().map(overrides);
            let availability = structs(handler, "defaultAvailability").next();
            for contract in CONTAINERS.iter().flat_map(|name| structs(handler, name)) {
                let Some(Value::Guid(id)) = contract.field("id") else {
                    continue;
                };
                let own = structs(contract, "paramOverrides").next().map(overrides);
                let mut insert = |sub: String, below: Option<&Overrides>| {
                    let levels = [below, own.as_ref(), handler_params.as_ref()];
                    from_handler_params +=
                        usize::from(handler_params.as_ref().is_some_and(states_cooldown));
                    expected.insert(
                        (record_guid.clone(), id.to_string(), sub),
                        resolve(levels, availability),
                    );
                };
                insert(String::new(), None);
                for sub in structs(contract, "subContracts") {
                    let Some(Value::Guid(sub_id)) = sub.field("id") else {
                        continue;
                    };
                    let params = structs(sub, "paramOverrides").next().unwrap_or(sub);
                    insert(sub_id.to_string(), Some(&overrides(params)));
                }
            }
        }
    }
    Ok((expected, from_handler_params))
}

fn same(actual: &Json, expected: &Json) -> bool {
    match (actual.as_f64(), expected.as_f64()) {
        (Some(actual), Some(expected)) => (actual - expected).abs() < 1e-4,
        _ => actual == expected,
    }
}

/// Asserts that `entry` carries the value and level of each of `fields` and the nearest level as
/// `cooldownLayer`.
fn check_entry(entry: &Json, key: &EntryKey, fields: &[Stated; 5]) -> TestResult {
    let layers = entry
        .get("cooldownLayers")
        .ok_or("actual cooldown layers")?;
    for ((name, _), stated) in FIELDS.iter().zip(fields) {
        let (level, value) = stated
            .as_ref()
            .map_or((Json::Null, Json::Null), |(level, value)| {
                (Json::from(*level), value.clone())
            });
        let actual_layer = v3::cell(layers, name)?.unwrap_or(&Json::Null);
        let actual_value = v3::cell(entry, name)?.unwrap_or(&Json::Null);
        assert_eq!(actual_layer, &level, "level of {name} of {key:?}");
        assert!(
            same(actual_value, &value),
            "{name} of {key:?}: {actual_value} against {value}"
        );
    }
    let nearest = LEVELS
        .iter()
        .find(|level| fields.iter().flatten().any(|(own, _)| own == *level));
    assert_eq!(
        cooldown_layer(entry)?,
        nearest.copied().unwrap_or("none"),
        "cooldownLayer of {key:?}"
    );
    Ok(())
}

fn cooldown_layer(entry: &Json) -> TestResult<&str> {
    Ok(v3::cell(entry, "cooldownLayer")?
        .map(|layer| layer.as_str().ok_or("cooldownLayer is a string"))
        .transpose()?
        .unwrap_or("none"))
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn every_cooldown_field_reports_the_stored_value_and_the_level_that_states_it() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, Some(real_archive_key()?))?;
    let (expected, from_handler_params) = expected_fields(&archive)?;
    let built = build_catalog(&archive, &CatalogOptions::new(SystemTime::now()))?;
    let catalog: Json = serde_json::from_slice(built.json())?;
    let contracts = v3::rows(&catalog)?;
    let projections = v3::Projections::read(&catalog)?;
    let mut census: BTreeMap<&str, usize> = BTreeMap::new();
    let mut checked = 0_usize;
    for entry in contracts {
        let layer = cooldown_layer(entry)?;
        *census.entry(layer).or_default() += 1;
        let (guid, sub) = v3::identity(entry)?;
        let key = (
            projections.header(entry, "guid")?.to_owned(),
            guid.to_owned(),
            sub.unwrap_or_default().to_owned(),
        );
        let fields = expected
            .get(&key)
            .ok_or("an entry without a stored contract")?;
        check_entry(entry, &key, fields)?;
        checked += 1;
    }
    print_line(&format!(
        "cooldownLayer census: {census:?}; {checked} entries checked; {from_handler_params} \
         entries under a handler-level override"
    ));
    assert!(
        census.contains_key("contract"),
        "a contract overrides a cooldown"
    );
    assert!(
        census.contains_key("subContract"),
        "a sub contract overrides a cooldown"
    );
    assert!(from_handler_params > 0, "a handler overrides a cooldown");
    assert_eq!(checked, contracts.len(), "every entry is checked");
    Ok(())
}
