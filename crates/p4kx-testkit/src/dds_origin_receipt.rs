// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;
use std::error::Error;
use std::str;

use crate::test_digest;
use serde_json::Value;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

/// Checks matching DDS bindings and exact ranges from plain Archive and observed loose inputs.
///
/// # Errors
/// Returns an error for a missing or invalid receipt field.
///
/// # Panics
/// Panics when a fixture has an incorrect binding, range, or source.
pub fn check_dds_origin_pair(
    payload: &[u8],
    archive_receipt: &[u8],
    file_receipt: &[u8],
) -> TestResult {
    let sha256 = test_digest::sha256_hex(payload)?;
    let expected_length = u64::try_from(payload.len())?;
    let archived = check_dds_origin(
        archive_receipt,
        &sha256,
        expected_length,
        "archiveEntry",
        &Value::Bool(false),
    )?;
    let loose = check_dds_origin(file_receipt, &sha256, expected_length, "file", &Value::Null)?;
    assert_eq!(
        archived, loose,
        "the same emitted ranges have different original inputs"
    );
    Ok(())
}

fn check_dds_origin(
    receipt: &[u8],
    sha256: &str,
    length: u64,
    source_kind: &str,
    marker: &Value,
) -> TestResult<Vec<Value>> {
    let rows = str::from_utf8(receipt)?
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<Vec<Value>, _>>()?;
    let binding = &rows.first().ok_or("DDS binding header")?["binding"];
    assert_eq!(
        binding["sha256"], sha256,
        "the receipt binds the written DDS"
    );
    assert_eq!(
        number(binding, "byteLength")?,
        length,
        "the exact emitted length"
    );
    let physical = rows
        .iter()
        .filter(|row| row["type"] == "source")
        .collect::<Vec<_>>();
    assert!(
        !physical.is_empty(),
        "physical source observations are present"
    );
    for source in physical {
        assert_eq!(
            source.pointer("/source/kind").and_then(Value::as_str),
            Some(source_kind),
            "the original input class"
        );
    }
    check_ranges(&rows, marker, length)
}

fn check_ranges(rows: &[Value], marker: &Value, length: u64) -> TestResult<Vec<Value>> {
    let ids = rows
        .iter()
        .filter_map(|row| row.pointer("/sourceUse/id").and_then(Value::as_str))
        .collect::<BTreeSet<_>>();
    let mut addresses = Vec::new();
    let mut next = 0;
    for field in rows.iter().filter(|row| row["type"] == "field") {
        assert_eq!(
            &field["containsEncryptedSource"], marker,
            "original encryption evidence"
        );
        assert_eq!(
            field["sourceSetComplete"], true,
            "every consumed physical input is known"
        );
        let uses = field["sourceUsesRefs"]
            .as_array()
            .ok_or("exact field contributors")?;
        assert!(!uses.is_empty(), "every range has a contributor");
        assert!(
            uses.iter()
                .all(|usage| usage.as_str().is_some_and(|id| ids.contains(id))),
            "every use resolves"
        );
        let address = &field["address"];
        let start = if addresses.is_empty() {
            assert_eq!(
                address["type"], "ddsHeaderBytes",
                "the first range is the header"
            );
            0
        } else {
            assert_eq!(
                address["type"], "ddsMipBytes",
                "remaining ranges address mips"
            );
            number(address, "start")?
        };
        let end = number(address, "endExclusive")?;
        assert_eq!(
            start, next,
            "the fields cover the merged bytes in output order"
        );
        assert!(end > start, "the synthetic DDS range is nonempty");
        next = end;
        addresses.push(address.clone());
    }
    assert_eq!(next, length, "the exact fields cover every output byte");
    Ok(addresses)
}

fn number(row: &Value, field: &str) -> TestResult<u64> {
    Ok(row[field]
        .as_str()
        .ok_or("canonical decimal coordinate")?
        .parse()?)
}
