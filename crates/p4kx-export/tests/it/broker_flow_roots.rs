// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::ExportHealthStatus;
use serde_json::Value;

use crate::broker_flow_graph::Broker;
use crate::flow_root_fixture::{self as fixture, Case};
use crate::health_origin_fixture::{self, Input, TestResult};

#[test]
fn broker_inside_and_overlapping_target_roots_use_the_same_original_fact_union() -> TestResult {
    let mut checked = Vec::new();
    for case in [Case::InsideFlow, Case::Overlap] {
        for kind in [Input::Plain, Input::Encrypted, Input::Standalone] {
            let source = fixture::open_broker(case, kind)?;
            let forge = source.entry.open()?;
            let record = forge.record(0).ok_or("actual original Broker root")?;
            assert_eq!(
                record.type_name()?,
                "MissionBrokerEntry",
                "the same genuine selected-field grammar has an actual Broker physical owner"
            );
            let original = crate::flow_root_source::check(&source, case, kind)?;
            checked.push((source, original, case, kind));
        }
    }
    for (source, original, case, kind) in checked {
        let built = p4kx_export::build_catalog_from(
            &source.archive,
            &source.entry,
            &health_origin_fixture::options(),
        )?;
        assert_eq!(
            built.summary().export_health().status,
            ExportHealthStatus::Complete,
            "only the actual selected same-owner union covers its genuine nonfinite facts"
        );
        let document: Value = serde_json::from_slice(built.json())?;
        let graph = Broker::for_guid(&document, fixture::GUID)?;
        crate::flow_root_output::check_owner(
            graph.definition,
            graph.configuration,
            built.summary().source_inventory(),
            &original,
            case,
            kind,
        )?;
        graph.projected(&document)?;
        crate::broker_flow_origins::payload_closure(
            graph.definition,
            built.summary().source_inventory(),
        )?;
        crate::health_origin_assertions::closure(&document)?;
    }
    Ok(())
}
