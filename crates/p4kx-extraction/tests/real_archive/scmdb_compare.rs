// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Compares Record JSON with an SCMDB data dump of the same Build; a development check, never an
//! export source (ADR-0267).
//!
//! Only values SCMDB takes from the DataForge unchanged are compared. Display names, texts,
//! densities and the maximum standing of a contract are SCMDB's own processing and left out.
#![expect(
    clippy::indexing_slicing,
    reason = "indexing a serde_json Value by key gives Null for a missing key and never panics"
)]

mod blueprints;
mod contracts;
mod findings;
mod input;
mod mining;
mod reward_coverage;
#[cfg(test)]
mod tests;
mod version;

use std::collections::{BTreeSet, HashMap};

use serde_json::Value;

pub(crate) use contracts::{ContractFields, collect_contracts};
pub(crate) use findings::Findings;
pub(crate) use version::{BLUEPRINTS, MERGED, MINING, dump_file_name, find_dump_version};

/// The three SCMDB dumps of one Build.
pub(crate) struct Dumps {
    /// `merged-<version>.json`: factions, standings and contracts.
    pub(crate) merged: Value,
    /// `crafting_blueprints-<version>.json`.
    pub(crate) blueprints: Value,
    /// `mining_data-<version>.json`.
    pub(crate) mining: Value,
}

/// The p4kx side of the comparison.
pub(crate) struct P4kx {
    /// Record JSON by the GUID of its Record, for every GUID of [`Dumps::record_guids`] that
    /// names a Record.
    pub(crate) records: HashMap<String, Value>,
    /// The compared fields of every contract of the contract Records, by its `id`.
    pub(crate) contracts: HashMap<String, ContractFields>,
}

impl Dumps {
    /// Returns the GUIDs of every Record the dumps describe.
    pub(crate) fn record_guids(&self) -> BTreeSet<&str> {
        let mut guids = BTreeSet::new();
        blueprints::record_guids(&self.blueprints, &mut guids);
        mining::record_guids(&self.mining, &mut guids);
        contracts::record_guids(&self.merged, &mut guids);
        guids
    }
}

/// Compares every 1:1 DataForge value of `dumps` with `p4kx`.
pub(crate) fn compare(dumps: &Dumps, p4kx: &P4kx) -> Findings {
    let mut findings = Findings::default();
    input::validate(dumps, &mut findings);
    blueprints::compare(&dumps.blueprints, &p4kx.records, &mut findings);
    mining::compare(&dumps.mining, &p4kx.records, &mut findings);
    contracts::compare(&dumps.merged, p4kx, &mut findings);
    reward_coverage::report(dumps, &mut findings);
    findings
}

/// Returns the GUID a Record JSON Reference links to.
fn reference_guid(reference: &Value) -> Option<&str> {
    reference["guid"].as_str()
}

/// Returns the elements of a JSON array, none for any other value.
fn elements(value: &Value) -> &[Value] {
    value.as_array().map_or(&[], Vec::as_slice)
}
