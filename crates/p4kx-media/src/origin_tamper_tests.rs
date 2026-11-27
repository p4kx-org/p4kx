// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::Value;

use crate::origin_read_support::{self as support, Fixture, TestResult};

#[test]
fn actual_reuse_rejects_scope_address_hash_counter_and_eof_tampering() -> TestResult {
    for mutation in [
        Mutation::Ordinal,
        Mutation::Stream,
        Mutation::Address,
        Mutation::PayloadHash,
        Mutation::Coverage,
        Mutation::Summary,
        Mutation::MissingEof,
        Mutation::TrailingRow,
        Mutation::Profile,
        Mutation::ReferenceClosure,
        Mutation::Marker,
    ] {
        rejects(mutation)?;
    }
    Ok(())
}

#[derive(Debug, Clone, Copy)]
enum Mutation {
    Ordinal,
    Stream,
    Address,
    PayloadHash,
    Coverage,
    Summary,
    MissingEof,
    TrailingRow,
    Profile,
    ReferenceClosure,
    Marker,
}

fn rejects(mutation: Mutation) -> TestResult {
    let fixture = Fixture::new()?;
    assert!(
        fixture.verify(&fixture.index).is_ok(),
        "positive control before {mutation:?}"
    );
    let mut rows = fixture.rows()?;
    apply(mutation, &mut rows)?;
    let mut index = fixture.index.clone();
    fixture.rewrite(&mut index, &rows)?;
    if matches!(mutation, Mutation::Summary) {
        let streams = first(&mut rows, "eof")?
            .get("streams")
            .ok_or("EOF summaries")?
            .clone();
        index.field_origins.as_mut().ok_or("current group")?.streams =
            serde_json::from_value(streams)?;
    }
    assert!(
        fixture.verify(&index).is_err(),
        "{mutation:?} must fail semantic verification after the edited receipt is rehashed"
    );
    Ok(())
}

fn apply(mutation: Mutation, rows: &mut Vec<Value>) -> TestResult {
    match mutation {
        Mutation::Ordinal => mutate_ordinal(rows)?,
        Mutation::Stream | Mutation::Address | Mutation::PayloadHash => {
            mutate_identity(mutation, rows)?;
        }
        Mutation::Coverage | Mutation::Summary => mutate_counter(mutation, rows)?,
        Mutation::MissingEof => rows.retain(|row| support::row_type(row) != Some("eof")),
        Mutation::TrailingRow => rows.push(serde_json::json!({"type":"eof","streams":[]})),
        Mutation::Profile => mutate_profile(rows)?,
        Mutation::ReferenceClosure | Mutation::Marker => mutate_origin(mutation, rows)?,
    }
    Ok(())
}

fn mutate_ordinal(rows: &mut [Value]) -> TestResult {
    for row in rows {
        if let Some(ordinal) = row.pointer_mut("/scope/ordinal") {
            *ordinal = Value::String("7".into());
        }
        if support::row_type(row) == Some("end") {
            return Ok(());
        }
    }
    Err("actual first occurrence end".into())
}
fn mutate_identity(mutation: Mutation, rows: &mut [Value]) -> TestResult {
    let (kind, pointer, value) = match mutation {
        Mutation::Stream => (
            "field",
            "/scope/stream",
            serde_json::json!({"type":"artifacts"}),
        ),
        Mutation::Address => ("field", "/field", Value::String("/unemitted~1datum".into())),
        Mutation::PayloadHash => (
            "occurrence",
            "/payload_sha256",
            Value::String("0".repeat(64)),
        ),
        _ => return Err("identity mutation variant".into()),
    };
    *first(rows, kind)?
        .pointer_mut(pointer)
        .ok_or("actual identity datum")? = value;
    Ok(())
}

fn mutate_counter(mutation: Mutation, rows: &mut [Value]) -> TestResult {
    let (kind, pointer, value) = match mutation {
        Mutation::Coverage => ("end", "/coverage/fields", "0"),
        Mutation::Summary => ("eof", "/streams/0/occurrences", "999"),
        _ => return Err("counter mutation variant".into()),
    };
    *first(rows, kind)?
        .pointer_mut(pointer)
        .ok_or("actual counter")? = Value::String(value.into());
    Ok(())
}

fn mutate_profile(rows: &mut [Value]) -> TestResult {
    let row = rows
        .iter_mut()
        .find(|row| {
            support::row_type(row) == Some("occurrence")
                && row.pointer("/scope/stream/type").and_then(Value::as_str) == Some("sources")
        })
        .ok_or("source occurrence")?;
    row.as_object_mut()
        .ok_or("header")?
        .insert("profile".into(), Value::String("archiveEntry".into()));
    Ok(())
}
fn mutate_origin(mutation: Mutation, rows: &mut [Value]) -> TestResult {
    let position = rows
        .iter()
        .position(|row| {
            support::row_type(row) == Some("field")
                && row
                    .get("sourceUsesRefs")
                    .and_then(Value::as_array)
                    .is_some_and(|refs| !refs.is_empty())
                && row.get("containsEncryptedSource") == Some(&Value::Bool(false))
        })
        .ok_or("known plain actual contributor")?;
    let row = rows.get_mut(position).ok_or("actual origin row")?;
    match mutation {
        Mutation::ReferenceClosure => {
            row.get_mut("sourceUsesRefs")
                .and_then(Value::as_array_mut)
                .ok_or("references")?
                .push(Value::String("missing-synthetic-use".into()));
        }
        Mutation::Marker => {
            *row.get_mut("containsEncryptedSource").ok_or("marker")? = Value::Bool(true);
            let end = rows
                .iter_mut()
                .skip(position + 1)
                .find(|row| support::row_type(row) == Some("end"))
                .ok_or("actual end")?;
            let counter = end
                .pointer_mut("/coverage/encrypted")
                .ok_or("encrypted counter")?;
            let count = counter.as_str().ok_or("canonical count")?.parse::<u64>()?;
            *counter = Value::String(count.checked_add(1).ok_or("counter overflow")?.to_string());
        }
        _ => return Err("origin mutation variant".into()),
    }
    Ok(())
}
fn first<'a>(
    rows: &'a mut [Value],
    kind: &str,
) -> Result<&'a mut Value, Box<dyn std::error::Error>> {
    rows.iter_mut()
        .find(|row| support::row_type(row) == Some(kind))
        .ok_or_else(|| format!("missing {kind} row").into())
}
