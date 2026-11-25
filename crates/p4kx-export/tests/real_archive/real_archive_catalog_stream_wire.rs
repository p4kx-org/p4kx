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

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum RowOrder {
    #[default]
    PhysicalSlot,
    Encounter(EncounterRow),
    Definition(DefinitionOwner),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum EncounterRow {
    Placement,
    MissionProperties,
    MissionFlows,
    MissionArena,
    LootOriginal,
    LootWeighted,
    HarvestSlotPreset,
}

impl EncounterRow {
    pub(super) fn project(self, row: Value) -> TestResult<(Fingerprint, Value)> {
        match self {
            Self::Placement => Ok((encounter_key(&row)?, row)),
            Self::LootOriginal | Self::LootWeighted | Self::HarvestSlotPreset => {
                Ok((super::loot_harvest::loot_harvest_key(self, &row)?, row))
            }
            _ => super::mission::project_mission_row(self, row),
        }
    }
}

#[derive(Default)]
pub(super) struct OrderedRows {
    order: RowOrder,
    previous: Option<PhysicalSlot>,
    count: usize,
    identities: Sha256,
    digest: Sha256,
}

impl OrderedRows {
    pub(super) fn with_definition(owner: DefinitionOwner) -> Self {
        Self {
            order: RowOrder::Definition(owner),
            ..Self::default()
        }
    }

    pub(super) fn in_encounter_order() -> Self {
        Self::with_encounter(EncounterRow::Placement)
    }

    pub(super) fn with_encounter(row: EncounterRow) -> Self {
        Self {
            order: RowOrder::Encounter(row),
            ..Self::default()
        }
    }

    pub(super) fn append(&mut self, slot: PhysicalSlot, row: Fingerprint) -> TestResult {
        if self.previous.is_some_and(|previous| previous >= slot) {
            return Err("physical component rows are unordered or duplicated".into());
        }
        self.previous = Some(slot);
        let record = slot.record_index.to_be_bytes();
        let component = slot.component_ordinal.to_be_bytes();
        self.identities.update(record);
        self.identities.update(component);
        self.digest.update(record);
        self.digest.update(component);
        self.append_row(row)
    }

    pub(super) fn append_encounter_row(
        &mut self,
        key: Fingerprint,
        row: Fingerprint,
    ) -> TestResult {
        self.identities.update(key);
        self.append_row(row)
    }

    fn append_row(&mut self, row: Fingerprint) -> TestResult {
        self.count = self
            .count
            .checked_add(1)
            .ok_or("row comparison counter overflow")?;
        self.digest.update(row);
        Ok(())
    }

    pub(super) fn finish(self) -> RowDigest {
        RowDigest {
            order: self.order,
            count: self.count,
            identities: self.identities.finalize().into(),
            digest: self.digest.finalize().into(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in super::super) struct RowDigest {
    order: RowOrder,
    pub(super) count: usize,
    identities: Fingerprint,
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
        formatter.write_str("a complete canonical row array in its original order")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Value, A::Error> {
        let mut ordered = OrderedRows {
            order: self.order,
            ..OrderedRows::default()
        };
        while let Some(row) = sequence.next_element::<Value>()? {
            if ordered.count == self.count {
                return Err(A::Error::custom(format!(
                    "extra retained row after {} expected rows",
                    self.count
                )));
            }
            match self.order {
                RowOrder::PhysicalSlot => {
                    let slot = wire_slot(&row).map_err(A::Error::custom)?;
                    ordered.append(slot, fingerprint(row).map_err(A::Error::custom)?)
                }
                RowOrder::Definition(owner) => {
                    let key = owner.key(&row).map_err(A::Error::custom)?;
                    ordered.append_encounter_row(key, fingerprint(row).map_err(A::Error::custom)?)
                }
                RowOrder::Encounter(shape) => {
                    let (key, row) = shape.project(row).map_err(A::Error::custom)?;
                    ordered.append_encounter_row(key, fingerprint(row).map_err(A::Error::custom)?)
                }
            }
            .map_err(A::Error::custom)?;
        }
        let observed = ordered.finish();
        if observed.count != self.count {
            return Err(A::Error::custom(format!(
                "retained row count differs: expected {}, observed {}",
                self.count, observed.count
            )));
        }
        if observed.identities != self.identities {
            return Err(A::Error::custom(
                "retained row identities or encounter order differ",
            ));
        }
        if observed.digest != self.digest {
            return Err(A::Error::custom(
                "retained row wire digests differ with matching identities and counts",
            ));
        }
        Ok(Value::from(self.count))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DefinitionOwner {
    Record,
    NestedRecord,
    Entry,
    CommodityPresentation,
}

impl DefinitionOwner {
    pub(super) fn key(self, row: &Value) -> TestResult<Fingerprint> {
        let digest = match self {
            Self::CommodityPresentation => {
                let Some([reference]) = row
                    .pointer("/commodityGuid/sourceUsesRefs")
                    .and_then(Value::as_array)
                    .map(Vec::as_slice)
                else {
                    return Err("the Commodity identity lacks one original header use".into());
                };
                let id = reference.as_str().ok_or("invalid Commodity header use")?;
                Sha256::digest(id.as_bytes())
            }
            Self::Record | Self::NestedRecord | Self::Entry => {
                Sha256::digest(self.physical_index(row)?.to_be_bytes())
            }
        };
        Ok(digest.into())
    }

    pub(crate) fn physical_index(self, row: &Value) -> TestResult<u32> {
        let position = match self {
            Self::Record => present_index(row.get("recordIndex")),
            Self::NestedRecord => present_index(row.pointer("/source/recordIndex")),
            Self::CommodityPresentation => None,
            Self::Entry => row.pointer("/identity/entryIndex"),
        };
        position
            .and_then(Value::as_u64)
            .and_then(|index| u32::try_from(index).ok())
            .ok_or_else(|| "missing or unreadable physical definition owner index".into())
    }
}

fn present_index(cell: Option<&Value>) -> Option<&Value> {
    cell.filter(|cell| cell.get("state").and_then(Value::as_str) == Some("present"))
        .and_then(|cell| cell.get("value"))
}

pub(super) fn encounter_key(row: &Value) -> TestResult<Fingerprint> {
    if row.pointer("/instanceId/state").and_then(Value::as_str) != Some("present") {
        return Err("the placement row lacks a present occurrence key".into());
    }
    let key = row
        .pointer("/instanceId/value")
        .filter(|value| value.is_object())
        .ok_or("the placement occurrence key is malformed")?;
    Ok(fingerprint(key.clone())?)
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
