// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::UNIX_EPOCH;

use p4kx_export::{CatalogOptions, ExportHealthStatus};
use serde_json::Value;

use crate::flow_root_fixture::{self as fixture, Case};
use crate::flow_root_output;
use crate::flow_root_source as source;
use crate::health_origin_fixture::{Input, TestResult};

#[test]
fn actual_inside_and_overlapping_source_roots_have_complete_original_controls() -> TestResult {
    for case in [Case::InsideFlow, Case::Overlap] {
        for input in [Input::Plain, Input::Encrypted, Input::Standalone] {
            let fixture = fixture::open(case, input)?;
            source::check(&fixture, case, input)?;
        }
    }
    Ok(())
}

#[test]
fn referenced_fields_already_inside_flow_alias_the_same_original_cells() -> TestResult {
    producer(Case::InsideFlow)
}

#[test]
fn overlapping_selected_subtrees_emit_each_original_value_once() -> TestResult {
    producer(Case::Overlap)
}

fn producer(case: Case) -> TestResult {
    let mut inputs = Vec::new();
    for input in [Input::Plain, Input::Encrypted, Input::Standalone] {
        let fixture = fixture::open(case, input)?;
        let original = source::check(&fixture, case, input)?;
        inputs.push((fixture, original, input));
    }
    for (fixture, original, input) in inputs {
        let built = p4kx_export::build_catalog_from(
            &fixture.archive,
            &fixture.entry,
            &CatalogOptions::new(UNIX_EPOCH),
        )?;
        assert_eq!(
            built.summary().export_health().status,
            ExportHealthStatus::Complete,
            "exact actually selected raw roots retain even the wrong-shaped id subtree's nonfinite number without a kind-wide waiver"
        );
        let catalog: Value = serde_json::from_slice(built.json())?;
        flow_root_output::check(
            &catalog,
            built.summary().source_inventory(),
            &original,
            case,
            input,
        )?;
    }
    Ok(())
}
