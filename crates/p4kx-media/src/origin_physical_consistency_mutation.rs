// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::Value;

use super::mutation::{self, Snapshot, Target};
use super::support::TestResult;
use crate::origin_inventory_wire::{Encryption, Scope};

pub(super) fn replace(snapshot: &mut Snapshot, pointer: &str, value: Value) -> TestResult {
    let source = leaf(snapshot)?;
    *source
        .pointer_mut(pointer)
        .ok_or("original typed source fact")? = value;
    Ok(())
}

pub(super) fn relabel_stored(snapshot: &mut Snapshot, state: Encryption) -> TestResult {
    let reference = leaf(snapshot)?
        .get("id")
        .and_then(Value::as_str)
        .ok_or("original source reference")?
        .to_owned();
    *leaf(snapshot)?
        .get_mut("storedEncryption")
        .ok_or("original stored state")? = Value::String(mutation::state_name(state).into());
    relabel_uses(snapshot, &reference, Some(Scope::Payload), state)
}

pub(super) fn crop_outer(
    snapshot: &mut Snapshot,
    expected: Encryption,
    forged: Encryption,
) -> TestResult {
    let physical_leaf = snapshot.source(Target::Leaf)?;
    let reference = physical_leaf
        .get("id")
        .and_then(Value::as_str)
        .ok_or("original leaf reference")?
        .to_owned();
    let ancestors = physical_leaf
        .get("ancestors")
        .and_then(Value::as_array)
        .ok_or("original ancestor chain")?;
    assert_eq!(
        ancestors.len(),
        2,
        "the genuine leaf has a grandparent and immediate parent"
    );
    let outer = ancestors.first().ok_or("original grandparent")?.clone();
    let parent = ancestors.last().ok_or("original parent")?.clone();
    let grandparent = snapshot
        .sources
        .iter()
        .find(|row| row.get("id") == Some(&outer))
        .ok_or("genuine grandparent description")?;
    assert_eq!(
        grandparent.get("storedEncryption").and_then(Value::as_str),
        Some(mutation::state_name(expected)),
        "the original outer witness is retained"
    );
    let parent_row = snapshot
        .sources
        .iter()
        .find(|row| row.get("id") == Some(&parent))
        .ok_or("genuine parent description")?;
    assert!(
        parent_row
            .get("ancestors")
            .and_then(Value::as_array)
            .is_some_and(|values| values.contains(&outer)),
        "the parent still proves the outer edge"
    );
    let sources_before = snapshot.sources.clone();
    *leaf(snapshot)?
        .get_mut("ancestors")
        .ok_or("leaf ancestor list")? = Value::Array(vec![parent]);
    relabel_uses(snapshot, &reference, None, forged)?;
    for (before, after) in sources_before.iter().zip(&snapshot.sources) {
        if before.get("id").and_then(Value::as_str) != Some(reference.as_str()) {
            assert_eq!(
                after, before,
                "all other original physical rows remain untouched"
            );
        }
    }
    Ok(())
}

pub(super) fn minimal(snapshot: &mut Snapshot) -> TestResult {
    let source = leaf(snapshot)?
        .as_object_mut()
        .ok_or("original source object")?;
    let index = source
        .get("entryIndex")
        .and_then(Value::as_u64)
        .ok_or("original numeric Entry index")?;
    source.retain(|key, _| matches!(key.as_str(), "id" | "ancestors" | "storedEncryption"));
    source.insert("kind".into(), Value::String("archiveEntryProof".into()));
    source.insert("entryIndex".into(), Value::String(index.to_string()));
    source.insert("details".into(), Value::String("captureLimited".into()));
    Ok(())
}

pub(super) fn fact(snapshot: &mut Snapshot, pointer: &str, value: Option<Value>) -> TestResult {
    let fact = leaf(snapshot)?
        .pointer_mut(pointer)
        .and_then(Value::as_object_mut)
        .ok_or("original sourced fact")?;
    let state = if value.is_some() { "present" } else { "absent" };
    *fact.get_mut("state").ok_or("explicit fact state")? = Value::String(state.into());
    *fact
        .get_mut("value")
        .ok_or("explicit nullable fact value")? = value.unwrap_or(Value::Null);
    Ok(())
}

fn leaf(snapshot: &mut Snapshot) -> Result<&mut Value, Box<dyn std::error::Error>> {
    snapshot
        .sources
        .iter_mut()
        .find(|source| source.pointer("/path/value").and_then(Value::as_str) == Some("leaf.ext"))
        .ok_or_else(|| "original full physical leaf".into())
}

fn relabel_uses(
    snapshot: &mut Snapshot,
    reference: &str,
    scope: Option<Scope>,
    state: Encryption,
) -> TestResult {
    let mut changed = 0_usize;
    for used in &mut snapshot.uses {
        if used.get("source").and_then(Value::as_str) != Some(reference) {
            continue;
        }
        let actual: Scope =
            serde_json::from_value(used.get("scope").ok_or("actual use scope")?.clone())?;
        if scope.is_some_and(|scope| scope != actual) {
            continue;
        }
        *used
            .get_mut("effectiveEncryption")
            .ok_or("original captured evidence")? =
            Value::String(mutation::state_name(state).into());
        changed = changed
            .checked_add(1)
            .ok_or("synthetic use count overflow")?;
    }
    assert!(
        changed > 0,
        "the mutation alters actual original-use observations"
    );
    Ok(())
}
