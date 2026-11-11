// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::ExportHealthStatus;
use p4kx_extraction::{DataType, Field, Value};
use serde_json::Value as Json;

use crate::broker_flow_failure_fixture as fixture;
use crate::broker_flow_graph::Broker;
use crate::health_origin_fixture::{Fixture, Input, TestResult};
use crate::template_flow_depth_fixture::{Branch, Tail};

#[test]
fn a_later_unknown_requirement_retains_all_emitted_flow_payloads_and_the_same_primary_failure()
-> TestResult {
    let mut checked = Vec::new();
    for kind in [Input::Plain, Input::Encrypted, Input::Standalone] {
        let source = fixture::open(kind)?;
        original(&source)?;
        checked.push((source, kind));
    }
    for (source, kind) in checked {
        crate::broker_flow_tests::refusal(&source);
        let (built, document) = source.diagnostic()?;
        assert_eq!(
            built.summary().export_health().status,
            ExportHealthStatus::Incomplete,
            "a later unsupported required context remains a failure"
        );
        let graph = Broker::for_guid(&document, crate::broker_flow_fixture::GUID)?;
        graph.chain(Branch::Not, 18, Tail::Known)?;
        check_flow_roots(&graph)?;
        crate::broker_flow_origins::check_owner(
            &graph,
            built.summary().source_inventory(),
            kind,
            fixture::FLOW_VALUES + 2,
        )?;
        crate::broker_flow_origins::unsupported_type(
            &built,
            kind,
            fixture::UNKNOWN_ORDINAL,
            fixture::OPERATION,
            fixture::UNKNOWN,
        )?;
        assert_eq!(
            built.summary().failures().len(),
            1,
            "the unchanged single primary failure"
        );
        assert!(built.summary().failures().first().ok_or("actual first Domain failure")?.reason()
            .contains("reputationRequirements holds the unknown struct SReputationMissionRequirementsUnknown"),
            "the later actual requirement keeps its exact type/field/cause instead of a flow failure");
        assert!(
            crate::broker_flow_graph::projections(&document)?.is_empty(),
            "no failed partial C projection"
        );
        crate::health_origin_assertions::closure(&document)?;
    }
    Ok(())
}

#[test]
fn a_failed_broker_meaning_keeps_its_own_original_nonflow_payload() -> TestResult {
    let mut checked = Vec::new();
    for kind in [Input::Plain, Input::Encrypted, Input::Standalone] {
        let source = fixture::open(kind)?;
        original(&source)?;
        checked.push((source, kind));
    }
    for (source, kind) in checked {
        crate::broker_flow_tests::refusal(&source);
        let (built, document) = source.diagnostic()?;
        let graph = Broker::for_guid(&document, crate::broker_flow_fixture::GUID)?;
        let payload = original_nonflow_payload(&graph)?;
        let input_ref = graph
            .definition
            .get("inputRef")
            .and_then(Json::as_str)
            .ok_or("actual original input")?;
        crate::flow_final_origin::Evidence {
            inventory: built.summary().source_inventory(),
            encrypted: kind.marker(),
            input_ref,
        }
        .value(
            payload.get("value").ok_or("actual original datum")?,
            fixture::UNKNOWN_ORDINAL + 1,
            "$value",
            p4kx_extraction::SourceUseRole::Payload,
        )?;
        crate::health_origin_assertions::closure(&document)?;
    }
    Ok(())
}

fn check_flow_roots(graph: &Broker<'_>) -> TestResult {
    let roots = crate::loot_v3_graph::array(graph.owner.source, "roots")?;
    assert_eq!(
        roots.len(),
        2,
        "both actual roots are retained even when the required C result fails"
    );
    let flow = roots.first().ok_or("actual original first Flow root")?;
    assert_eq!(
        flow.get("sourcePosition"),
        Some(&serde_json::json!({"parentOrdinal":null,"fieldIndex":"0"})),
        "the original Flow retains its independently proven physical root position"
    );
    let later = graph.owner.follow(
        roots
            .get(1)
            .and_then(|root| root.get("value"))
            .ok_or("actual second root value link")?,
    )?;
    assert_eq!(
        crate::flow_owner_graph::ordinal(later)?,
        u64::try_from(fixture::FLOW_VALUES)?,
        "the later original root follows exactly the complete original Flow subtree"
    );
    Ok(())
}

fn original_nonflow_payload<'catalog>(graph: &Broker<'catalog>) -> TestResult<&'catalog Json> {
    let root = crate::loot_v3_graph::array(graph.owner.source, "roots")?;
    assert_eq!(
        root.len(),
        2,
        "both genuine original root declarations are retained even after C refusal"
    );
    let original = root.get(1).ok_or("actual later requirement root")?;
    assert_eq!(
        original.pointer("/name/value"),
        Some(&serde_json::json!("reputationRequirements")),
        "the exact later original declaration"
    );
    assert_eq!(
        original.get("sourcePosition"),
        Some(&serde_json::json!({"parentOrdinal":null,"fieldIndex":"1"})),
        "the actual original root position remains independent of local navigation"
    );
    let value = graph.owner.follow(
        original
            .get("value")
            .ok_or("actual retained requirement link")?,
    )?;
    let fields = crate::loot_v3_graph::structure(value, fixture::UNKNOWN)?;
    assert_eq!(
        fields.len(),
        1,
        "the original unsupported Struct has its one actual member"
    );
    let payload = graph.owner.follow(
        fields
            .first()
            .and_then(|field| field.get("value"))
            .ok_or("actual original unsupported value field")?,
    )?;
    assert_eq!(
        payload.pointer("/value/value/value"),
        Some(&serde_json::json!({"type":"uint64","value":u64::MAX.to_string()})),
        "the exact authored UInt64 maximum survives without a fake successful interpretation"
    );
    Ok(payload)
}

fn original(source: &Fixture) -> TestResult {
    crate::template_flow_depth_source::check_owner(
        source,
        Branch::Not,
        18,
        Tail::Known,
        "MissionBrokerEntry",
        crate::broker_flow_fixture::GUID,
    )?;
    let forge = source.entry.open()?;
    let record = forge.record(0).ok_or("actual physical Broker source")?;
    assert_eq!(forge.records().len(), 1, "one actual physical source owner");
    let tree = record.tree()?;
    assert_eq!(
        tree.error_count(),
        0,
        "the later unsupported source is fully readable"
    );
    let names: Vec<_> = tree.fields().iter().map(Field::name).collect();
    assert_eq!(
        names,
        ["missionFlow", "reputationRequirements"],
        "the exact actual original declarations"
    );
    let field = tree
        .fields()
        .get(1)
        .ok_or("actual later requirement field")?;
    assert_eq!(
        field.storage().data_type(),
        DataType::StrongPointer,
        "real selected pointer storage"
    );
    assert!(
        !field.storage().is_array(),
        "the actual later requirement is scalar"
    );
    let Value::Struct(unknown) = field.value() else {
        return Err("actual readable unknown Struct".into());
    };
    assert_eq!(
        unknown.type_name(),
        fixture::UNKNOWN,
        "the actual original unsupported concrete type"
    );
    assert_eq!(
        unknown.fields().len(),
        1,
        "the complete extra original source payload"
    );
    assert_eq!(
        unknown.field("originalPayload"),
        Some(&Value::UInt64(u64::MAX)),
        "the exact original UInt64 maximum is not fabricated by an output DTO"
    );
    Ok(())
}
