// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::path::Path;

use serde_json::{Value, json};

use super::TestResult;

pub(super) const STRING_TYPE: u32 = 0xAABB_CCDD;
pub(super) const NAME_HEADER_BYTES: usize = 5;
pub(super) const FIRST_NAME_HEADER: usize = 36;
pub(super) const NAMES: [(u32, &[u8]); 4] = [
    (77, b" a\0name "),
    (77, b"ok\xff"),
    (88, b"\xe2\x82"),
    (99, b""),
];
pub(super) const TRAILING: &[u8] = &[0xc1, 0xc2];

pub(super) fn soundbank(
    count: u32,
    names: &[(u32, &[u8])],
    trailing: &[u8],
) -> TestResult<Vec<u8>> {
    let mut bank = Vec::new();
    let header = [154_u32, 13, 0]
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect::<Vec<_>>();
    section(&mut bank, *b"BKHD", &header)?;
    let mut stid = [STRING_TYPE, count]
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect::<Vec<_>>();
    for (id, name) in names {
        stid.extend_from_slice(&id.to_le_bytes());
        stid.push(u8::try_from(name.len())?);
        stid.extend_from_slice(name);
    }
    stid.extend_from_slice(trailing);
    section(&mut bank, *b"STID", &stid)?;
    Ok(bank)
}

fn section(bank: &mut Vec<u8>, tag: [u8; 4], body: &[u8]) -> TestResult {
    bank.extend_from_slice(&tag);
    bank.extend_from_slice(&u32::try_from(body.len())?.to_le_bytes());
    bank.extend_from_slice(body);
    Ok(())
}

pub(super) fn range(start: usize, end: usize) -> Value {
    json!({"basis":{"type":"entry"},"start":start.to_string(),"end_exclusive":end.to_string()})
}

pub(super) fn check_origins(base: &Path, record: usize) -> TestResult {
    let fields = rows(base, "field-origins.jsonl")?;
    let uses = rows(base, "origin-uses.jsonl")?;
    let position = record.to_string();
    let start = FIRST_NAME_HEADER + 2 * NAME_HEADER_BYTES + NAMES[0].1.len();
    for (field, role, site, offset, name) in [
        (
            "/data/payload/name_bytes/2",
            "payload",
            "bnkByte",
            start + 2,
            "stidNameByte",
        ),
        (
            "/data/payload/name_text/encoding",
            "conversion",
            "bnkStidName",
            start,
            "nameBytes",
        ),
    ] {
        let receipt = fields
            .iter()
            .find(|row| {
                row.get("field").and_then(Value::as_str) == Some(field)
                    && row.pointer("/scope/stream/kind").and_then(Value::as_str) == Some("audio")
                    && row.pointer("/scope/ordinal").and_then(Value::as_str)
                        == Some(position.as_str())
            })
            .ok_or("actual STID origin")?;
        assert_eq!(
            receipt.get("containsEncryptedSource"),
            Some(&json!(true)),
            "each value binds the actual encrypted BNK"
        );
        assert_eq!(
            receipt.get("sourceSetComplete"),
            Some(&json!(true)),
            "actual STID source use is known"
        );
        let refs = receipt
            .get("sourceUsesRefs")
            .and_then(Value::as_array)
            .ok_or("STID source-use references")?;
        let location = json!({"type":"subresource","namespace":site,"indices":[offset.to_string()],"field":name});
        assert!(
            uses.iter().any(|used| {
                used.get("id").is_some_and(|id| refs.contains(id))
                    && used.get("role").and_then(Value::as_str) == Some(role)
                    && used.get("location") == Some(&location)
            }),
            "exact original STID byte or conversion site for {field}"
        );
    }
    Ok(())
}

fn rows(directory: &Path, file: &str) -> TestResult<Vec<Value>> {
    fs::read_to_string(directory.join(file))?
        .lines()
        .map(|line| serde_json::from_str(line).map_err(Into::into))
        .collect()
}
