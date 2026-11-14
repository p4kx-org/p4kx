// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Contracts in the Basetool catalog of a real Archive; runs with `--ignored` when `P4KX_DATA_P4K`
//! names one.

use std::collections::{BTreeMap, HashSet};
use std::error::Error;
use std::time::SystemTime;

use p4kx_export::{CatalogOptions, Kind, build_catalog};
use p4kx_extraction::Archive;
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::Value;

use crate::catalog_v3_consumer;

#[path = "../contract_v3/mod.rs"]
pub(crate) mod v3;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

struct Contract<'a> {
    row: &'a Value,
    guid: &'a str,
    sub: Option<&'a str>,
    title: &'a str,
    path: &'a str,
    gains: &'a [Value],
    negative_gains: usize,
}

fn contracts(catalog: &Value) -> TestResult<Vec<Contract<'_>>> {
    let projections = v3::Projections::read(catalog)?;
    v3::rows(catalog)?
        .iter()
        .map(|row| {
            let (guid, sub) = v3::identity(row)?;
            let gains = reputation_gains(row)?;
            let mut negative_gains = 0;
            for gain in gains {
                negative_gains +=
                    usize::from(reputation_gain_amount(gain)?.is_some_and(|amount| amount < 0));
            }
            Ok(Contract {
                row,
                guid,
                sub,
                title: projections.text(row, "title")?.en.unwrap_or_default(),
                path: projections.header(row, "recordPath")?,
                gains,
                negative_gains,
            })
        })
        .collect()
}

pub(super) fn reputation_gains(row: &Value) -> TestResult<&[Value]> {
    let carrier = row
        .get("reputationGains")
        .ok_or("required reputation gains carrier")?;
    catalog_v3_consumer::present(carrier)?
        .as_array()
        .map(Vec::as_slice)
        .ok_or_else(|| "reputation gains must be an array".into())
}

pub(super) fn reputation_gain_text<'row>(
    gain: &'row Value,
    field: &str,
) -> TestResult<Option<&'row str>> {
    let carrier = gain
        .get(field)
        .ok_or_else(|| format!("required gain field {field}"))?;
    catalog_v3_consumer::source_value(carrier)?
        .map(|value| {
            value
                .as_str()
                .ok_or_else(|| format!("{field} must be a string").into())
        })
        .transpose()
}

pub(super) fn reputation_gain_amount(gain: &Value) -> TestResult<Option<i64>> {
    reputation_gain_text(gain, "amount")?
        .map(|text| {
            let amount: i64 = text.parse()?;
            if amount.to_string() != text {
                return Err("reputation amount must be canonical signed decimal text".into());
            }
            Ok(amount)
        })
        .transpose()
}

const SAMPLES: usize = 25;
const NAMED: [&str; 3] = ["Eckhart_Nyx_VLRT_DefendShip", "RCD_Contested", "Hockrow"];

fn text<'c>(entry: &'c Value, field: &str) -> &'c str {
    entry
        .get(field)
        .and_then(|text| text.get("en"))
        .and_then(Value::as_str)
        .unwrap_or_default()
}

#[expect(
    clippy::print_stdout,
    reason = "the figures are the result of the verification"
)]
fn gaps(contracts: &[Contract]) {
    let describe = |label: &str, gap: &dyn Fn(&Contract) -> bool| {
        let missing: Vec<&Contract> = contracts.iter().filter(|entry| gap(entry)).collect();
        let no_template = missing
            .iter()
            .filter(|entry| entry.row["templateGuid"].is_null())
            .count();
        let mut by_handler: BTreeMap<&str, usize> = BTreeMap::new();
        let mut by_template: BTreeMap<&str, usize> = BTreeMap::new();
        for entry in &missing {
            *by_handler
                .entry(entry.row["handler"].as_str().unwrap_or_default())
                .or_default() += 1;
            *by_template
                .entry(entry.row["templateGuid"].as_str().unwrap_or("none"))
                .or_default() += 1;
        }
        println!(
            "{label}: {} entries, {no_template} without a template; handlers {:?}; templates {:?}",
            missing.len(),
            by_handler.iter().take(15).collect::<Vec<_>>(),
            by_template.iter().take(15).collect::<Vec<_>>()
        );
        for entry in missing.iter().take(8) {
            println!(
                "  {} | {} | {} | {}",
                entry.row["debugName"], entry.row["handler"], entry.row["templateGuid"], entry.path
            );
        }
    };
    describe("untitled", &|entry| entry.title.is_empty());
    describe("untyped", &|entry| entry.row["missionTypeGuid"].is_null());
    describe("unresolved tokens", &|entry| {
        let title = entry.title;
        title.contains("[CONTRACTOR]") || title.contains("[RACETYPE]") || title.contains("[ITEM]")
    });
    describe("no pickup places", &|entry| {
        entry.row["pickupPlaces"]
            .as_array()
            .is_none_or(Vec::is_empty)
    });
    describe("negative gains", &|entry| entry.negative_gains > 0);
}

#[expect(
    clippy::print_stdout,
    reason = "the figures are the result of the verification"
)]
fn report(contracts: &[Contract]) -> TestResult {
    let subs = contracts.iter().filter(|entry| entry.sub.is_some()).count();
    println!("{} entries, {subs} of them sub contracts", contracts.len());
    let mut placeholders: BTreeMap<&str, usize> = BTreeMap::new();
    let (mut untitled, mut untyped, mut gained, mut negative) = (0, 0, 0, 0);
    for entry in contracts {
        let title = entry.title;
        untitled += usize::from(title.is_empty());
        untyped += usize::from(entry.row["missionTypeGuid"].is_null());
        gained += usize::from(!entry.gains.is_empty());
        negative += entry.negative_gains;
        for part in title.split('[').skip(1) {
            if let Some((name, _)) = part.split_once(']') {
                *placeholders.entry(name).or_default() += 1;
            }
        }
    }
    println!("{untitled} without a title, {untyped} without a mission type");
    println!("{gained} with reputation gains, {negative} negative amounts among them");
    println!("placeholders in titles: {placeholders:?}");
    gaps(contracts);
    for entry in contracts.iter().take(SAMPLES) {
        println!(
            "{} | {:?} | {} | {} | {} | {}",
            entry.row["debugName"],
            entry.sub,
            entry.title,
            text(entry.row, "missionType"),
            entry.row["pickupPlaces"],
            serde_json::to_string(entry.gains)?
        );
    }
    for name in NAMED {
        for entry in contracts
            .iter()
            .filter(|entry| {
                entry.row["debugName"]
                    .as_str()
                    .is_some_and(|debug| debug.contains(name))
            })
            .take(6)
        {
            println!(
                "{name}: {} | {:?} | {} | {}",
                entry.row["debugName"],
                entry.sub,
                entry.title,
                serde_json::to_string(entry.gains)?
            );
        }
    }
    Ok(())
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
#[expect(
    clippy::print_stdout,
    reason = "the figures are the result of the verification"
)]
fn the_contracts_of_a_real_archive_resolve_titles_types_and_success_reputation() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, Some(real_archive_key()?))?;
    let built = build_catalog(&archive, &CatalogOptions::new(SystemTime::now()))?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let contracts = contracts(&catalog)?;
    report(&contracts)?;
    let summary = built.summary().kind(Kind::Contracts);
    println!(
        "summary: {summary:?}, failures: {}",
        built.summary().failures().len()
    );

    assert!(!contracts.is_empty(), "contracts");
    assert_eq!(summary.map(|kind| kind.count), Some(contracts.len()));
    assert!(
        contracts.iter().any(|entry| entry.sub.is_some()),
        "sub contracts"
    );
    let keys: HashSet<_> = contracts
        .iter()
        .map(|entry| (entry.guid, entry.sub))
        .collect();
    assert_eq!(
        keys.len(),
        contracts.len(),
        "one entry per contract and sub contract"
    );
    assert!(
        contracts
            .iter()
            .all(|entry| !entry.title.contains("~mission(")),
        "every token is replaced"
    );
    let untitled = contracts
        .iter()
        .filter(|entry| entry.title.is_empty())
        .count();
    let untyped = contracts
        .iter()
        .filter(|entry| entry.row["missionTypeGuid"].is_null())
        .count();
    assert!(untitled * 100 < contracts.len(), "{untitled} untitled");
    assert!(untyped * 100 < contracts.len(), "{untyped} untyped");
    Ok(())
}
