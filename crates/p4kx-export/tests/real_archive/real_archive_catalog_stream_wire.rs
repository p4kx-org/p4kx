// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;
use std::cmp::Ordering;
use std::fmt;
use std::fs::File;
use std::io::{self, BufReader};

use serde::de::{DeserializeSeed, Deserializer, Error as _, SeqAccess, Visitor};
use serde_json::Value;
use sha2::{Digest as _, Sha256};

use super::super::TestResult;
use super::super::digest_parity::{PhysicalSlot, fingerprint};
use super::super::selected_origins::SelectOrigin;
use super::digest_index::DigestIndex;
use super::digest_runs::{Fingerprint, OriginDigest};

#[derive(Default)]
pub(super) struct OrderedRows {
    previous: Option<PhysicalSlot>,
    count: usize,
    digest: Sha256,
}

impl OrderedRows {
    pub(super) fn append(&mut self, slot: PhysicalSlot, row: Fingerprint) -> TestResult {
        if self.previous.is_some_and(|previous| previous >= slot) {
            return Err("physical component rows are unordered or duplicated".into());
        }
        self.previous = Some(slot);
        self.count = self
            .count
            .checked_add(1)
            .ok_or("component row counter overflow")?;
        self.digest.update(slot.record_index.to_be_bytes());
        self.digest.update(slot.component_ordinal.to_be_bytes());
        self.digest.update(row);
        Ok(())
    }

    pub(super) fn finish(self) -> RowDigest {
        RowDigest {
            count: self.count,
            digest: self.digest.finalize().into(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in super::super) struct RowDigest {
    pub(super) count: usize,
    digest: Fingerprint,
}

impl<'de> DeserializeSeed<'de> for RowDigest {
    type Value = Value;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Value, D::Error> {
        deserializer.deserialize_seq(self)
    }
}

impl<'de> Visitor<'de> for RowDigest {
    type Value = Value;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a complete ordered physical component row array")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Value, A::Error> {
        let mut ordered = OrderedRows::default();
        while let Some(row) = sequence.next_element::<Value>()? {
            if ordered.count == self.count {
                return Err(A::Error::custom("extra retained physical component row"));
            }
            let slot = wire_slot(&row).map_err(A::Error::custom)?;
            ordered
                .append(slot, fingerprint(row).map_err(A::Error::custom)?)
                .map_err(A::Error::custom)?;
        }
        if ordered.finish() != self {
            return Err(A::Error::custom(
                "retained physical component order, count or wire digest differs",
            ));
        }
        Ok(Value::from(self.count))
    }
}

fn wire_slot(row: &Value) -> TestResult<PhysicalSlot> {
    let field = |key| -> TestResult<u32> {
        let cell = row.get(key).ok_or("missing physical ownership cell")?;
        if cell.get("state").and_then(Value::as_str) != Some("present") {
            return Err("the physical ownership cell is not present".into());
        }
        cell.get("value")
            .and_then(Value::as_u64)
            .and_then(|value| u32::try_from(value).ok())
            .ok_or_else(|| "invalid physical ownership index".into())
    };
    Ok(PhysicalSlot {
        record_index: field("ownerRecordIndex")?,
        component_ordinal: field("componentOrdinal")?,
    })
}

#[derive(Clone, Copy)]
pub(in super::super) struct StreamOrigins<'scope>(pub(super) &'scope DigestIndex);

impl<'de> DeserializeSeed<'de> for StreamOrigins<'_> {
    type Value = Value;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Value, D::Error> {
        deserializer.deserialize_seq(self)
    }
}

impl<'de> Visitor<'de> for StreamOrigins<'_> {
    type Value = Value;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("the ordered Catalog provenance inventory")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Value, A::Error> {
        let state = RefCell::new(OriginProbe::new(self.0).map_err(A::Error::custom)?);
        let selects = |id: &str| state.borrow_mut().select(id);
        let mut compared = 0_u64;
        while let Some(row) = sequence.next_element_seed(SelectOrigin(&selects))? {
            let mut probe = state.borrow_mut();
            if let Some(error) = probe.error.take() {
                return Err(A::Error::custom(error));
            }
            match (probe.expected.take(), row) {
                (Some(expected), Some((_, row))) => {
                    if fingerprint(row).map_err(A::Error::custom)? != expected {
                        return Err(A::Error::custom(
                            "Catalog and SDK source observation digests differ",
                        ));
                    }
                    compared = compared
                        .checked_add(1)
                        .ok_or_else(|| A::Error::custom("source digest counter overflow"))?;
                }
                (None, None) => {}
                _ => return Err(A::Error::custom("source observation selection changed")),
            }
        }
        if compared != self.0.count() {
            return Err(A::Error::custom(
                "missing Catalog source observation needed by the SDK",
            ));
        }
        Ok(Value::from(compared))
    }
}

struct OriginProbe<'scope> {
    index: &'scope DigestIndex,
    file: BufReader<File>,
    previous: String,
    expected: Option<Fingerprint>,
    error: Option<io::Error>,
}

impl<'scope> OriginProbe<'scope> {
    fn new(index: &'scope DigestIndex) -> io::Result<Self> {
        Ok(Self {
            index,
            file: index.reader()?,
            previous: String::new(),
            expected: None,
            error: None,
        })
    }

    fn select(&mut self, id: &str) -> bool {
        match self.observe(id) {
            Ok(expected) => {
                self.expected = expected;
                self.expected.is_some()
            }
            Err(error) => {
                self.error = Some(error);
                false
            }
        }
    }

    fn observe(&mut self, id: &str) -> io::Result<Option<Fingerprint>> {
        if self.previous.as_str().cmp(id) != Ordering::Less {
            return Err(io::Error::other(
                "Catalog source observation IDs are unordered or duplicated",
            ));
        }
        self.previous.clear();
        self.previous.push_str(id);
        self.index
            .lookup(&mut self.file, &OriginDigest::id_hash(id))
    }
}

#[cfg(test)]
#[path = "real_archive_catalog_stream_wire_tests.rs"]
mod tests;
