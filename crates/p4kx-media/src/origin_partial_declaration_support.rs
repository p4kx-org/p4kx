// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{ArchiveBuilder, TestEntry};
use serde_json::Value;

pub(super) use super::super::consistency_mutation as changes;
pub(super) use super::super::mutation::Snapshot;
pub(super) use super::super::support::{Fixture, Scene, TestResult};
use crate::MediaError;
pub(super) use crate::origin_inventory_wire::Encryption;

pub(super) const DECLARATION: &str = "/declarations/value/0/value";
const CONTRADICTION: &str = "stored source encryption contradicts available directory declarations";

pub(super) fn checked(fixture: &Fixture) -> Result<Snapshot, Box<dyn std::error::Error>> {
    assert!(
        fixture.reuse()?.is_some(),
        "the actual producer generation is reusable"
    );
    Snapshot::load(fixture)
}

pub(super) fn leaf(snapshot: &mut Snapshot) -> Result<&mut Value, Box<dyn std::error::Error>> {
    snapshot
        .sources
        .iter_mut()
        .find(|row| row.pointer("/path/value").and_then(Value::as_str) == Some("leaf.ext"))
        .ok_or_else(|| "the actual full physical leaf".into())
}

pub(super) fn unavailable_occurrence(snapshot: &mut Snapshot) -> TestResult {
    let values = leaf(snapshot)?
        .pointer_mut("/declarations/value")
        .and_then(Value::as_array_mut)
        .ok_or("actual declaration occurrences")?;
    let mut missing = values.first().ok_or("original declaration")?.clone();
    *missing.get_mut("state").ok_or("declaration state")? = Value::String("unreadable".into());
    *missing.get_mut("value").ok_or("declaration value")? = Value::Null;
    values.push(missing);
    Ok(())
}

pub(super) fn unreadable(snapshot: &mut Snapshot, field: &str) -> TestResult {
    unavailable(snapshot, &format!("{DECLARATION}/{field}"))
}

pub(super) fn unavailable(snapshot: &mut Snapshot, pointer: &str) -> TestResult {
    fact_state(snapshot, pointer, "unreadable")
}

pub(super) fn fact_state(snapshot: &mut Snapshot, pointer: &str, state: &str) -> TestResult {
    let fact = leaf(snapshot)?
        .pointer_mut(pointer)
        .and_then(Value::as_object_mut)
        .ok_or("the original typed fact")?;
    *fact.get_mut("state").ok_or("the original fact state")? = Value::String(state.into());
    *fact
        .get_mut("value")
        .ok_or("the original nullable fact value")? = Value::Null;
    Ok(())
}

pub(super) fn unknown(bytes: Option<Vec<u8>>) -> Result<Fixture, Box<dyn std::error::Error>> {
    let entry = TestEntry::zstd("leaf.ext", b"original synthetic bytes").encrypted();
    let entry = match bytes {
        Some(bytes) => entry.with_encryption_block(bytes),
        None => entry.with_encryption_field(7),
    };
    Fixture::from_builder(&ArchiveBuilder::new().entry(entry))
}

pub(super) fn reject(fixture: &mut Fixture, snapshot: &mut Snapshot) -> TestResult {
    snapshot.publish_forgery(fixture)?;
    let result = fixture.reuse();
    assert!(
        matches!(&result, Err(MediaError::Destination(detail)) if detail == CONTRADICTION),
        "fully rebound impossible declaration evidence must fail the intended consistency check: {result:?}"
    );
    Ok(())
}

pub(super) fn accept(fixture: &mut Fixture, snapshot: &mut Snapshot) -> TestResult {
    snapshot.publish_forgery(fixture)?;
    assert!(
        fixture.reuse()?.is_some(),
        "possible partial evidence retains the original proof"
    );
    Ok(())
}
