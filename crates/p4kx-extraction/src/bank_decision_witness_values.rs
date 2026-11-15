// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::ops::Range;

use p4kx_audio::BankHircField;
use serde_json::{Value, json};

use super::{TestResult, require};

pub(super) struct Projection<'a> {
    pub(super) document: &'a Value,
    body: &'a [u8],
    body_start: u64,
    protected: Option<bool>,
    source: &'a str,
    uses: BTreeMap<&'a str, &'a Value>,
}

impl<'a> Projection<'a> {
    pub(super) fn new(
        document: &'a Value,
        body: &'a [u8],
        body_start: u64,
        protected: Option<bool>,
        entry_ordinal: u32,
    ) -> TestResult<Self> {
        let source = original_source(document, entry_ordinal)?;
        let rows = document
            .pointer("/source_inventory/uses")
            .and_then(Value::as_array)
            .ok_or("original bank source inventory")?;
        let mut uses = BTreeMap::new();
        for row in rows {
            let id = row
                .get("id")
                .and_then(Value::as_str)
                .ok_or("source use identity")?;
            require(
                uses.insert(id, row).is_none(),
                "unique source use in the view",
            )?;
        }
        Ok(Self {
            document,
            body,
            body_start,
            protected,
            source,
            uses,
        })
    }

    pub(super) fn field<T: Word>(
        &self,
        row: &Value,
        path: &str,
        field: &BankHircField<T>,
    ) -> TestResult {
        let value = row.pointer(path).ok_or("original source word is emitted")?;
        self.value(value, &field.value.wire())?;
        self.receipt(value, &field.range, "payload")?;
        let start = self.relative(field.range.start)?;
        let end = self.relative(field.range.end)?;
        require(
            self.body
                .get(start..end)
                .is_some_and(|bytes| field.value.matches_bytes(bytes)),
            "original word matches bytes in the verified bank",
        )
    }

    pub(super) fn optional<T: Word>(
        &self,
        row: &Value,
        path: &str,
        field: Option<&BankHircField<T>>,
    ) -> TestResult {
        field.map_or_else(
            || {
                require(
                    row.pointer(path).is_none(),
                    "unread source word remains absent",
                )
            },
            |field| self.field(row, path, field),
        )
    }

    pub(super) fn at(&self, row: &Value, path: &str, expected: &Value) -> TestResult {
        let datum = row.pointer(path).ok_or("sourced original property")?;
        self.value(datum, expected)?;
        for reference in source_references(datum)? {
            self.resolve(reference)?;
        }
        Ok(())
    }

    fn value(&self, datum: &Value, expected: &Value) -> TestResult {
        require(
            datum.get("value") == Some(expected),
            "authored value or physical ordinal is preserved",
        )?;
        require(
            datum.get("containsEncryptedSource") == Some(&json!(self.protected))
                && datum.get("sourceSetComplete") == Some(&Value::Bool(true)),
            "immediate datum provenance matches the proved Archive contribution",
        )
    }

    pub(super) fn receipt(&self, datum: &Value, range: &Range<u64>, role: &str) -> TestResult {
        let offsets = json!([range.start.to_string(), range.end.to_string()]);
        let mut present = false;
        for reference in source_references(datum)? {
            let receipt = self.resolve(reference)?;
            if receipt.get("role").and_then(Value::as_str) == Some(role)
                && receipt.get("scope").and_then(Value::as_str) == Some("payload")
                && receipt.pointer("/location/indices") == Some(&offsets)
            {
                present = true;
            }
        }
        require(
            present,
            "datum receipt addresses the exact original byte range",
        )
    }

    fn resolve(&self, reference: &Value) -> TestResult<&'a Value> {
        let id = reference.as_str().ok_or("source use identity")?;
        let receipt = self
            .uses
            .get(id)
            .copied()
            .ok_or("datum reference joins inventory")?;
        require(
            receipt.get("source").and_then(Value::as_str) == Some(self.source),
            "datum receipt belongs to the exact physical Bank Entry",
        )?;
        Ok(receipt)
    }

    fn relative(&self, offset: u64) -> TestResult<usize> {
        Ok(usize::try_from(
            offset
                .checked_sub(self.body_start)
                .ok_or("word remains inside its physical owner")?,
        )?)
    }

    pub(super) fn rows(&self, path: &str) -> TestResult<Rows<'a>> {
        let array = self
            .document
            .pointer(path)
            .and_then(Value::as_array)
            .ok_or("projected ordered rows")?;
        Ok(Rows(array.iter()))
    }
}

fn source_references(datum: &Value) -> TestResult<&[Value]> {
    datum
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .filter(|references| !references.is_empty())
        .map(Vec::as_slice)
        .ok_or_else(|| "immediate source use references".into())
}

fn original_source(document: &Value, entry_ordinal: u32) -> TestResult<&str> {
    let sources = document
        .pointer("/source_inventory/sources")
        .and_then(Value::as_array)
        .ok_or("original physical source inventory")?;
    let ordinal_text = entry_ordinal.to_string();
    let mut matches = sources.iter().filter(|source| {
        let ordinal = source.get("entryIndex");
        match source.get("kind").and_then(Value::as_str) {
            Some("archiveEntry") => {
                ordinal.and_then(Value::as_u64) == Some(u64::from(entry_ordinal))
            }
            Some("archiveEntryProof") => {
                ordinal.and_then(Value::as_str) == Some(ordinal_text.as_str())
            }
            _ => false,
        }
    });
    let source = matches.next().ok_or("original physical Archive Entry")?;
    require(matches.next().is_none(), "unique physical Archive Entry")?;
    source
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| "physical Archive Entry reference".into())
}

pub(super) struct Rows<'a>(std::slice::Iter<'a, Value>);

impl<'a> Rows<'a> {
    pub(super) fn one(&mut self) -> TestResult<&'a Value> {
        self.0
            .next()
            .ok_or_else(|| "one row for each original occurrence".into())
    }

    pub(super) fn kind(&mut self, kind: &str) -> TestResult<&'a Value> {
        let row = self.one()?;
        require(
            row.get("kind").and_then(Value::as_str) == Some(kind),
            "projection keeps original row kinds and order",
        )?;
        Ok(row)
    }

    pub(super) fn finish(mut self) -> TestResult {
        require(
            self.0.next().is_none(),
            "projection emits no extra occurrences",
        )
    }
}

pub(super) trait Word: Copy {
    fn matches_bytes(self, bytes: &[u8]) -> bool;
    fn wire(self) -> Value;
}

macro_rules! word {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl Word for $ty {
                fn matches_bytes(self, bytes: &[u8]) -> bool {
                    self.to_le_bytes().as_slice() == bytes
                }
                fn wire(self) -> Value {
                    json!(self)
                }
            }
        )+
    };
}

word!(u8, u16, u32);

impl Word for u64 {
    fn matches_bytes(self, bytes: &[u8]) -> bool {
        self.to_le_bytes().as_slice() == bytes
    }

    fn wire(self) -> Value {
        json!(format!("{self:016x}"))
    }
}
