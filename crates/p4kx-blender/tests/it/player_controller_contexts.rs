// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;

use p4kx_assembly::BodyVariant;
use p4kx_blender::{PlayerSource, write_player_source_bundle};
use p4kx_extraction::CancelToken;
use p4kx_testkit::{CryXmlByteOrder, CryXmlSignature, TestElement, TestEntry};
use serde_json::{Value, json};

use super::TestResult;

fn controller() -> TestEntry {
    let contexts = TestElement::new("ScopeContextDefs")
        .child(TestElement::new("Main").attribute("ordering", "synthetic"))
        .child(TestElement::new("Double"))
        .child(TestElement::new("Double"));
    let scopes = TestElement::new("ScopeDefs")
        .child(
            TestElement::new("Body")
                .attribute("context", "Main")
                .attribute("layer", "0")
                .attribute("alwaysRootScope", "1"),
        )
        .child(TestElement::new("Head").attribute("context", "Double"))
        .child(TestElement::new("Missing").attribute("context", "Unknown"))
        .child(TestElement::new("Empty").attribute("context", ""))
        .child(TestElement::new("Omitted"))
        .child(
            TestElement::new("Invalid")
                .attribute("context", "Main")
                .attribute("context", "Double"),
        );
    let fragment = TestElement::new("Walk")
        .attribute("scopes", "Head+Body++Ghost")
        .child(
            TestElement::new("Override")
                .attribute("tags", "heavy")
                .attribute("fragTags", "local")
                .attribute("scopes", "Body+Head"),
        );
    let document = TestElement::new("ControllerDef")
        .child(TestElement::new("Fragments").attribute("filename", "Animations/Test/fragments.xml"))
        .child(contexts)
        .child(scopes)
        .child(TestElement::new("FragmentDefs").child(fragment))
        .to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little);
    TestEntry::stored("Data/Animations/test/controller.xml", document).encrypted()
}

#[test]
fn controller_contexts_resolve_original_scopes_without_claiming_a_rig() -> TestResult {
    let [_, adb, fragment] = super::motion_option_assertions::entries("");
    let (_fixture, archive) = super::fixture_with_motion([controller(), adb, fragment], [])?;
    let directory = tempfile::tempdir()?;
    let output = directory.path().join("topology");
    let cancel = CancelToken::new();
    let bodies = [BodyVariant::Female, BodyVariant::Male];
    let old = serde_json::to_value(PlayerSource::read(&archive, bodies[0], &cancel)?)?;
    write_player_source_bundle(&archive, &bodies, &output, &cancel)?;
    let rows = fs::read_to_string(output.join("records.jsonl"))?
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<Vec<Value>, _>>()?;
    let projections: Vec<_> = rows
        .iter()
        .filter(|row| row["type"] == "controllerTopology")
        .collect();
    let [topology] = projections.as_slice() else {
        return Err("one shared original controller topology for both bodies".into());
    };
    let motion = rows
        .iter()
        .find(|row| row["type"] == "motionControllerTopology")
        .ok_or("component topology binding")?;
    verify_owner(topology, motion);
    verify_scopes(&topology["value"])?;
    verify_fragments(&topology["value"])?;
    let after = serde_json::to_value(PlayerSource::read(&archive, bodies[0], &cancel)?)?;
    assert_eq!(
        after, old,
        "the legacy projection and its evidence IDs remain identical"
    );
    Ok(())
}

fn verify_owner(topology: &Value, motion: &Value) {
    assert_eq!(
        motion["component"],
        json!(0),
        "same component order as the other rows"
    );
    assert_eq!(
        motion.pointer("/value/value/controllerDocument"),
        topology.pointer("/value/document"),
        "reference uses exact shared physical document"
    );
    assert_eq!(
        motion.pointer("/value/value/status"),
        Some(&json!("unresolved")),
        "controller context alone does not select a rig"
    );
    assert_eq!(
        motion.pointer("/value/containsEncryptedSource"),
        Some(&json!(true)),
        "actual encrypted controller evidence survives the unresolved owner"
    );
    assert_eq!(
        motion.pointer("/value/sourceSetComplete"),
        Some(&json!(false)),
        "the original input dependency is still missing"
    );
}

fn verify_scopes(topology: &Value) -> TestResult {
    let contexts = topology["contexts"].as_array().ok_or("contexts")?;
    assert_eq!(
        contexts.len(),
        3,
        "scope context source order preserves duplicates"
    );
    assert_eq!(
        contexts
            .first()
            .and_then(|context| context.pointer("/name/value")),
        Some(&json!("Main")),
        "first authored context remains first"
    );
    let scopes = topology["scopes"].as_array().ok_or("scopes")?;
    let [body, head, missing, empty, omitted, invalid] = scopes.as_slice() else {
        return Err("all six authored scopes".into());
    };
    for (scope, status, candidates) in [
        (body, "resolved", json!([0])),
        (head, "ambiguous", json!([1, 2])),
        (missing, "missing", json!([])),
        (empty, "emptySelector", json!([])),
    ] {
        assert_eq!(
            scope.pointer("/context/value/status"),
            Some(&json!(status)),
            "authored context disposition"
        );
        assert_eq!(
            scope.pointer("/context/value/candidates"),
            Some(&candidates),
            "ordered exact context declaration indexes"
        );
    }
    assert_eq!(
        omitted.pointer("/context/state"),
        Some(&json!("absent")),
        "omitted context differs from an explicit empty value"
    );
    assert_eq!(
        invalid.pointer("/context/state"),
        Some(&json!("invalid")),
        "duplicate context attributes must never select one"
    );
    assert_eq!(
        invalid
            .pointer("/declaration/attributes")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(2),
        "both duplicate attribute coordinates remain"
    );
    assert_eq!(
        body.pointer("/declaration/attributes/2/value/value"),
        Some(&json!("1")),
        "alwaysRootScope is present as its original data"
    );
    Ok(())
}

fn verify_fragments(topology: &Value) -> TestResult {
    let fragment = topology
        .pointer("/fragments/0")
        .ok_or("fragment declaration")?;
    let scopes = fragment["scopes"].as_array().ok_or("fragment scopes")?;
    let [head, body, empty, unmatched] = scopes.as_slice() else {
        return Err("all authored scope token occurrences".into());
    };
    assert_eq!(
        head.pointer("/value/candidates"),
        Some(&json!([1])),
        "first token keeps authored selector order rather than scope-definition order"
    );
    assert_eq!(
        body.pointer("/value/candidates"),
        Some(&json!([0])),
        "second token selects original body scope"
    );
    assert_eq!(
        empty.pointer("/value/status"),
        Some(&json!("emptySelector")),
        "empty occurrence remains visible"
    );
    assert_eq!(
        unmatched.pointer("/value/status"),
        Some(&json!("missing")),
        "missing scope visible"
    );
    assert_eq!(
        fragment.pointer("/overrides/0/scopes/0/value/candidates"),
        Some(&json!([0])),
        "override has its own exact scope"
    );
    assert_eq!(
        fragment.pointer("/overrides/0/declaration/attributes/1/value/value"),
        Some(&json!("local")),
        "override tags are authored values"
    );
    Ok(())
}
