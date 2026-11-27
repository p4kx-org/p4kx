// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet, HashMap};

use p4kx_extraction::Guid;
use serde_json::Value;

use super::{Findings, P4kx, elements, reference_guid};

const STANDINGS: &str = "standings";
const LISTS: [(&str, &str); 2] = [
    ("contracts", "contracts"),
    ("legacyContracts", "legacy contracts"),
];
const STANDING_SIDES: [&str; 2] = ["minStanding", "maxStanding"];

/// Every encountered compared projection of one contract ID.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct ContractFields {
    occurrences: Vec<ContractOccurrence>,
}

#[derive(Debug, PartialEq, Eq)]
struct ContractOccurrence {
    min_standing: Option<String>,
    results: Option<Value>,
}

impl ContractOccurrence {
    fn of(contract: &serde_json::Map<String, Value>) -> Self {
        let results = contract.get("contractResults");
        Self {
            min_standing: contract
                .get("minStanding")
                .and_then(reference_guid)
                .map(str::to_owned),
            results: results.cloned(),
        }
    }
}

/// Collects authored ID occurrences without choosing a duplicate winner.
pub(crate) fn collect_contracts(record: &Value, contracts: &mut HashMap<String, ContractFields>) {
    let mut pending = vec![record];
    while let Some(value) = pending.pop() {
        match value {
            Value::Object(object) => {
                if let Some(id) = object.get("id").and_then(Value::as_str)
                    && id.parse::<Guid>().is_ok()
                {
                    contracts
                        .entry(id.to_owned())
                        .or_default()
                        .occurrences
                        .push(ContractOccurrence::of(object));
                }
                pending.extend(object.values().rev());
            }
            Value::Array(values) => pending.extend(values.iter().rev()),
            _ => {}
        }
    }
}

pub(super) fn record_guids<'d>(merged: &'d Value, guids: &mut BTreeSet<&'d str>) {
    guids.extend(standings(merged).into_keys());
}

pub(super) fn compare(merged: &Value, p4kx: &P4kx, findings: &mut Findings) {
    for (list, category) in LISTS {
        for scmdb in elements(&merged[list]) {
            compare_contract(category, scmdb, &p4kx.contracts, findings);
        }
    }
    for (guid, scmdb) in standings(merged) {
        let ident = scmdb["name"].as_str().unwrap_or(guid);
        let record = p4kx.records.get(guid);
        findings.check(STANDINGS, record.is_some(), || {
            format!("{ident}: {guid} is no Record")
        });
        if let Some(record) = record {
            findings.integer(
                STANDINGS,
                ident,
                "minReputation",
                (scmdb.get("minReputation"), record.get("minReputation")),
            );
        }
    }
}

fn compare_contract(
    category: &'static str,
    scmdb: &Value,
    contracts: &HashMap<String, ContractFields>,
    findings: &mut Findings,
) {
    let id = scmdb["id"].as_str().unwrap_or_default();
    let ident = scmdb["debugName"].as_str().unwrap_or(id);
    let contract = contracts.get(id);
    findings.check(category, contract.is_some(), || {
        format!("{ident}: id {id} is in no contract Record")
    });
    let Some(contract) = contract else { return };
    let Some(first) = contract.occurrences.first() else {
        return;
    };
    let consistent = contract
        .occurrences
        .iter()
        .all(|occurrence| occurrence == first);
    findings.check(category, consistent, || {
        format!(
            "{ident}: {} conflicting source occurrences of id {id}",
            contract.occurrences.len()
        )
    });
    if !consistent {
        return;
    }
    findings.number_if_both(
        category,
        ident,
        "timeToComplete",
        scmdb["timeToComplete"].as_f64(),
        first
            .results
            .as_ref()
            .and_then(|value| value["timeToComplete"].as_f64()),
    );
    let buy_in = first
        .results
        .as_ref()
        .and_then(|value| value.get("contractBuyInAmount"));
    findings.integer_if_both(category, ident, "buyIn", (scmdb.get("buyIn"), buy_in));
    findings.text_if_both(
        category,
        ident,
        "minStanding",
        scmdb["minStanding"]["guid"].as_str(),
        first.min_standing.as_deref(),
    );
}

/// Returns mentioned standing objects by their GUID.
fn standings(merged: &Value) -> BTreeMap<&str, &Value> {
    LISTS
        .iter()
        .flat_map(|(list, _)| elements(&merged[*list]))
        .flat_map(|contract| STANDING_SIDES.map(|side| &contract[side]))
        .filter_map(|standing| Some((standing["guid"].as_str()?, standing)))
        .collect()
}
