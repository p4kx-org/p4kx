// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Differential verification of P4KX against the output of unp4k, unforge and `StarBreaker`.
//!
//! The ignored tests read a folder of reference output the owner produced beforehand, named by
//! `P4KX_REF_UNP4K`, `P4KX_REF_UNFORGE` or `P4KX_REF_STARBREAKER`, next to the real Archive of
//! `P4KX_DATA_P4K`; they never run a foreign tool. The mutation self-tests run everywhere.

use std::error::Error;

mod bytes;
mod manifest_fixtures;
mod mutation_allow;
mod mutation_bytes;
mod mutation_embedded;
mod mutation_manifest_coverage;
mod mutation_manifest_logs;
mod mutation_manifest_paths;
mod mutation_manifest_prepare;
mod mutation_manifest_records;
mod mutation_manifest_trust;
mod mutation_unp4k;
mod mutation_value;
mod mutation_xml;
mod reference_artifacts;
mod reference_digest;
mod reference_error;
mod reference_identity;
mod reference_inventory;
mod reference_layout;
mod reference_limits;
mod reference_load;
mod reference_log_report;
mod reference_log_scan;
mod reference_logs;
mod reference_manifest;
mod reference_mappings;
mod reference_paths;
mod reference_prepare;
mod reference_provenance;
mod reference_reader;
mod reference_records;
mod reference_run;
mod reference_schema;
mod reference_source;
mod samples;
mod starbreaker;
mod starbreaker_json;
mod starbreaker_location;
mod tally;
mod unforge;
mod unp4k;
mod unp4k_allow;
mod value_tree;
mod value_tree_compare;
mod xml_tree;

type TestResult = Result<(), Box<dyn Error>>;

#[expect(
    clippy::print_stdout,
    reason = "the counts and timings of a differential run are what the verification reports"
)]
fn report(line: &str) {
    println!("{line}");
}
