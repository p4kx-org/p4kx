// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use serde_json::Value;

use super::{TestResult, origin};

const GAME_GRAPH: &str = "/definitions/weaponAnimationSources";
const REPEATS: usize = 64;

pub(super) fn check(catalog: &Value, candidates: &[Value]) -> TestResult {
    let graph = origin::at(catalog, GAME_GRAPH)?;
    let legacy = candidates
        .iter()
        .map(|row| legacy_row(catalog, graph, row))
        .collect::<TestResult<Vec<_>>>()?;
    compare_repeated_bytes(candidates, &legacy)
}

fn legacy_row(catalog: &Value, graph: &Value, row: &Value) -> TestResult<Value> {
    let evidence = origin::cell(row, "tagEvidence", "present")?;
    assert_eq!(
        evidence["value"], "sharedDeclaration",
        "the unchanged candidate establishes the scoped tag match"
    );
    let inputs = evidence_inputs(catalog, graph, row)?;
    let expected = exact_references(&inputs)?;
    assert_eq!(
        origin::rows(evidence, "/sourceUsesRefs")?
            .iter()
            .map(reference)
            .collect::<TestResult<Vec<_>>>()?,
        expected.into_iter().collect::<Vec<_>>(),
        "the candidate retains exactly the original sorted and deduplicated use references"
    );
    check_markers(evidence, &inputs);
    let playback = origin::at(row, "/playback")?;
    assert_eq!(
        playback, "unverified",
        "the status remains a tool-owned control value"
    );
    let mut legacy = row.clone();
    let mut legacy_playback = evidence.clone();
    *legacy_playback
        .get_mut("value")
        .ok_or("the original sourced candidate value")? = playback.clone();
    *legacy.get_mut("playback").ok_or("the candidate status")? = legacy_playback;
    Ok(legacy)
}

fn evidence_inputs<'data>(
    catalog: &'data Value,
    graph: &'data Value,
    candidate: &'data Value,
) -> TestResult<Vec<&'data Value>> {
    let weapon = weapon_attachment(catalog, candidate)?;
    let actor = unique(origin::rows(graph, "/actorBindings")?, |actor| {
        actor.get("record") == candidate.get("playerRecordIndex")
            && actor.get("component") == candidate.get("playerComponentOrdinal")
    })?;
    let branch = unique(origin::rows(graph, "/branches")?, |branch| {
        branch.get("source") == candidate.get("branchDatabase")
            && branch.get("position") == candidate.get("branchPosition")
    })?;
    let option = database_option(graph, candidate)?;
    let tags = unique(origin::rows(option, "/attributes")?, |attr| {
        attr.pointer("/name/value").and_then(Value::as_str) == Some("Tags")
            && attr.pointer("/namespace/value") == Some(&Value::Null)
    })?;
    let target = origin::at(candidate, "/optionDatabase")?;
    let scope = unique(origin::rows(graph, "/scopes")?, |scope| {
        scope.get("database") == Some(target)
    })?;
    let mut inputs = vec![
        origin::at(weapon, "/itemType")?,
        origin::at(weapon, "/tags")?,
        origin::at(actor, "/record")?,
        origin::at(actor, "/component")?,
        origin::at(branch, "/tags")?,
        origin::at(branch, "/target/state")?,
        origin::at(tags, "/value")?,
        origin::at(scope, "/tags/coverage")?,
    ];
    append_fragments(graph, candidate, &mut inputs)?;
    for matched in origin::rows(candidate, "/matches")? {
        inputs.push(origin::at(matched, "/declaration")?);
    }
    Ok(inputs)
}

fn weapon_attachment<'data>(catalog: &'data Value, candidate: &Value) -> TestResult<&'data Value> {
    let owner = unique(origin::rows(catalog, "/definitions/items/rows")?, |item| {
        item.pointer("/recordIndex/value") == candidate.pointer("/weaponRecordIndex/value")
    })?;
    let component = unique(origin::rows(owner, "/components/value")?, |component| {
        component.pointer("/componentOrdinal/value")
            == candidate.pointer("/attachmentComponentOrdinal/value")
    })?;
    origin::at(component, "/value/value/weaponMotion")
}

fn database_option<'data>(graph: &'data Value, candidate: &Value) -> TestResult<&'data Value> {
    let target = origin::at(candidate, "/optionDatabase")?;
    let database = unique(origin::rows(graph, "/databases")?, |database| {
        database.pointer("/source/inputRef") == target.get("inputRef")
            && database.pointer("/source/entryIndex") == target.get("entryIndex")
    })?;
    unique(origin::rows(database, "/options")?, |option| {
        option.get("position") == candidate.get("optionPosition")
    })
}

fn append_fragments<'data>(
    graph: &'data Value,
    candidate: &Value,
    inputs: &mut Vec<&'data Value>,
) -> TestResult {
    for row in origin::rows(graph, "/fragmentSelectors")? {
        if row.get("source") == candidate.get("branchDatabase")
            && row.get("branch") == candidate.get("branchPosition")
            && row.pointer("/status/value").and_then(Value::as_str) == Some("matched")
            && origin::rows(row, "/options")?
                .iter()
                .any(|option| option.get("value") == candidate.pointer("/optionPosition/value"))
        {
            inputs.push(origin::at(row, "/status")?);
        }
    }
    Ok(())
}

fn unique(rows: &[Value], predicate: impl Fn(&Value) -> bool) -> TestResult<&Value> {
    let mut matches = rows.iter().filter(|row| predicate(row));
    let row = matches
        .next()
        .ok_or("the original candidate operand is missing")?;
    if matches.next().is_some() {
        return Err("the original candidate operand is ambiguous".into());
    }
    Ok(row)
}

fn exact_references<'data>(inputs: &[&'data Value]) -> TestResult<BTreeSet<&'data str>> {
    let mut references = BTreeSet::new();
    for input in inputs {
        for used in origin::rows(input, "/sourceUsesRefs")? {
            references.insert(reference(used)?);
        }
    }
    Ok(references)
}

fn reference(use_id: &Value) -> TestResult<&str> {
    Ok(use_id
        .as_str()
        .ok_or("expected an original source-use ID")?)
}

fn check_markers(evidence: &Value, inputs: &[&Value]) {
    let complete = inputs
        .iter()
        .all(|input| input["sourceSetComplete"] == true);
    let encrypted = if inputs
        .iter()
        .any(|input| input["containsEncryptedSource"] == true)
    {
        Some(true)
    } else if complete
        && inputs
            .iter()
            .all(|input| input["containsEncryptedSource"] == false)
    {
        Some(false)
    } else {
        None
    };
    assert_eq!(
        evidence["sourceSetComplete"], complete,
        "the proof remains as complete as its original contributions"
    );
    assert_eq!(
        evidence["containsEncryptedSource"],
        Value::from(encrypted),
        "the conservative immediate encryption marker follows the exact contributors"
    );
}

fn compare_repeated_bytes(compact: &[Value], legacy: &[Value]) -> TestResult {
    let repeated = |rows: &[Value]| {
        serde_json::to_vec(
            &rows
                .iter()
                .cycle()
                .take(rows.len() * REPEATS)
                .collect::<Vec<_>>(),
        )
    };
    let previous_bytes = repeated(legacy)?.len();
    let compact_bytes = repeated(compact)?.len();
    let mut playback_bytes = 0;
    for (previous, current) in legacy.iter().zip(compact) {
        let previous = serde_json::to_vec(origin::at(previous, "/playback")?)?.len();
        let current = serde_json::to_vec(origin::at(current, "/playback")?)?.len();
        playback_bytes += previous
            .checked_sub(current)
            .ok_or("the canonical control grew")?;
    }
    assert!(
        playback_bytes > 0,
        "the original provenance wrapper was repeated"
    );
    assert_eq!(
        previous_bytes.checked_sub(compact_bytes),
        Some(playback_bytes * REPEATS),
        "the repeated candidate array saves exactly the redundant status wrappers"
    );
    Ok(())
}
