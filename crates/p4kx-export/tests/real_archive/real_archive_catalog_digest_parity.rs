// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Borrow;
use std::collections::btree_map::Entry as SlotEntry;
use std::collections::hash_map::Entry as DigestEntry;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt;
use std::hash::Hash;

use p4kx_export::catalog_queries::DatasetAvailability;
use p4kx_extraction::{PhysicalSourceRef, SourceInventory, SourceUseRole};
use serde::Serialize;
use serde::de::{DeserializeSeed, Deserializer, Error as _, IgnoredAny, SeqAccess, Visitor};
use serde_json::Value;
use sha2::{Digest as _, Sha256};

use super::compare_rows::Projection;
use super::selected_origins::SelectOrigin;
use super::wire_row;
use super::{BoundMeasurement, Counts, SOURCES, TestResult, USES, collect_references, measurement};

pub(super) type WireDigest = [u8; 32];
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct PhysicalSlot {
    pub(crate) record_index: u32,
    pub(crate) component_ordinal: u32,
}

pub(super) fn fingerprint(mut value: Value) -> serde_json::Result<WireDigest> {
    value.sort_all_objects();
    Ok(Sha256::digest(serde_json::to_vec(&value)?).into())
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Ownership {
    Direct,
    Dependency,
}

#[derive(Default)]
pub(crate) struct DigestedTable {
    rows: BTreeMap<PhysicalSlot, (Ownership, WireDigest)>,
    direct_rows: usize,
}

impl DigestedTable {
    pub(crate) fn retain_direct(
        &mut self,
        slot: PhysicalSlot,
        row: &impl Serialize,
        references: &mut HashSet<String>,
    ) -> TestResult {
        if self
            .rows
            .last_key_value()
            .is_some_and(|(last, _)| *last >= slot)
        {
            return Err(format!(
                "the standalone SDK physical rows are unordered or duplicated at {slot:?}"
            )
            .into());
        }
        let digest = row_fingerprint(row, references)?;
        self.rows.insert(slot, (Ownership::Direct, digest));
        self.direct_rows += 1;
        Ok(())
    }

    pub(crate) fn retain_dependency(
        &mut self,
        slot: PhysicalSlot,
        row: &impl Serialize,
        references: &mut HashSet<String>,
    ) -> TestResult {
        let digest = row_fingerprint(row, references)?;
        match self.rows.entry(slot) {
            SlotEntry::Vacant(entry) => {
                entry.insert((Ownership::Dependency, digest));
            }
            SlotEntry::Occupied(entry) if entry.get() == &(Ownership::Dependency, digest) => {}
            SlotEntry::Occupied(_) => {
                return Err(format!(
                    "a linked dependency conflicts with another physical row at {slot:?}"
                )
                .into());
            }
        }
        Ok(())
    }

    pub(crate) fn dependency_count(&self) -> usize {
        self.rows.len() - self.direct_rows
    }
}

pub(super) fn row_fingerprint(
    row: &impl Serialize,
    references: &mut HashSet<String>,
) -> TestResult<WireDigest> {
    let row = wire_row(row)?;
    let mut needed = HashSet::new();
    collect_references(&row, &mut needed);
    if needed.is_empty() {
        return Err("a physical row lacks contributing source uses".into());
    }
    for id in needed {
        if !references.contains(id) {
            references.insert(id.to_owned());
        }
    }
    Ok(fingerprint(row)?)
}

#[derive(Default)]
pub(crate) struct DigestedInventory {
    uses: HashMap<String, WireDigest>,
    sources: HashMap<String, WireDigest>,
}

impl DigestedInventory {
    pub(crate) fn retain(
        &mut self,
        inventory: &SourceInventory,
        references: HashSet<String>,
    ) -> TestResult {
        capture_inventory(
            inventory,
            references,
            |id, row| retain_fingerprint(&mut self.uses, id, row),
            |id, row| retain_fingerprint(&mut self.sources, id, row),
        )
    }
}

fn retain_fingerprint(
    digests: &mut HashMap<String, WireDigest>,
    id: String,
    row: Value,
) -> TestResult {
    let digest = fingerprint(row)?;
    match digests.entry(id) {
        DigestEntry::Vacant(entry) => {
            entry.insert(digest);
        }
        DigestEntry::Occupied(entry) if entry.get() == &digest => {}
        DigestEntry::Occupied(_) => {
            return Err("the SDK produced conflicting source observations".into());
        }
    }
    Ok(())
}

pub(super) fn capture_inventory<Id: Borrow<str> + Eq + Hash>(
    inventory: &SourceInventory,
    mut references: HashSet<Id>,
    mut retain_use: impl FnMut(String, Value) -> TestResult,
    mut retain_source: impl FnMut(String, Value) -> TestResult,
) -> TestResult {
    if references.is_empty() {
        return Err("expected original cell provenance".into());
    }
    let mut pending = Vec::new();
    for used in inventory.iter_uses() {
        let id = used.id().as_str();
        if !references.remove(id) {
            continue;
        }
        if !matches!(
            used.role(),
            SourceUseRole::Payload | SourceUseRole::Selector | SourceUseRole::Conversion
        ) {
            return Err("a value refers to a noncontributing observation".into());
        }
        retain_use(id.to_owned(), serde_json::to_value(used)?)?;
        pending.push(used.source().as_str());
    }
    if !references.is_empty() {
        return Err("a value lacks its referenced SDK source use".into());
    }

    let sources: HashMap<_, _> = inventory
        .iter_sources()
        .map(|source| (source.id().as_str(), source))
        .collect();
    let mut visited = HashSet::new();
    while let Some(id) = pending.pop() {
        if !visited.insert(id) {
            continue;
        }
        let source = sources
            .get(id)
            .ok_or("missing SDK physical source or ancestor")?;
        pending.extend(source.ancestors().iter().map(PhysicalSourceRef::as_str));
        retain_source(id.to_owned(), serde_json::to_value(source)?)?;
    }
    Ok(())
}

impl BoundMeasurement {
    pub(crate) fn verify_digested_tables<const TABLES: usize>(
        &self,
        paths: [&str; TABLES],
        tables: &[DigestedTable; TABLES],
        inventory: &DigestedInventory,
    ) -> TestResult<Counts<TABLES>> {
        for (path, table) in paths.iter().zip(tables) {
            if table.rows.is_empty() {
                return Err(format!("the parity route has no rows at {path}").into());
            }
        }
        self.verify_digested_tables_with_availability(
            paths,
            tables,
            [DatasetAvailability::Available; TABLES],
            inventory,
        )
    }

    pub(crate) fn verify_digested_tables_with_availability<const TABLES: usize>(
        &self,
        paths: [&str; TABLES],
        tables: &[DigestedTable; TABLES],
        availability: [DatasetAvailability; TABLES],
        inventory: &DigestedInventory,
    ) -> TestResult<Counts<TABLES>> {
        self.watch.check()?;
        let mut capture = measurement::normal_capture();
        for ((path, table), expected) in paths.iter().zip(tables).zip(availability) {
            require_digest_scope(path, table, expected)?;
            capture.keep(&format!("{path}/availability"));
            capture.project(
                &format!("{path}/rows"),
                Projection::DigestedRows(DigestedRows(path, table)),
            );
        }
        capture.project(
            USES,
            Projection::DigestedOrigins(DigestedOrigins(&inventory.uses)),
        );
        capture.project(
            SOURCES,
            Projection::DigestedOrigins(DigestedOrigins(&inventory.sources)),
        );
        let catalog = self.read_catalog(&capture)?;
        for ((path, table), expected) in paths.iter().zip(tables).zip(availability) {
            let actual = catalog.pointer(path).ok_or("required full Catalog table")?;
            let expected = serde_json::to_value(expected)?;
            if actual.get("availability") != Some(&expected)
                || actual.get("rows") != Some(&Value::from(table.rows.len()))
            {
                return Err(format!("Catalog and SDK table scope differs at {path}").into());
            }
        }
        for (path, expected) in [(USES, &inventory.uses), (SOURCES, &inventory.sources)] {
            if catalog.pointer(path) != Some(&Value::from(expected.len())) {
                return Err(
                    format!("Catalog and SDK contributing provenance differs at {path}").into(),
                );
            }
        }
        self.watch.check()?;
        Ok(Counts {
            rows: tables.each_ref().map(|table| table.rows.len()),
            source_uses: inventory.uses.len(),
            physical_sources: inventory.sources.len(),
        })
    }
}

fn require_digest_scope(
    path: &str,
    table: &DigestedTable,
    availability: DatasetAvailability,
) -> TestResult {
    match availability {
        DatasetAvailability::Available => Ok(()),
        DatasetAvailability::Absent if table.rows.is_empty() => Ok(()),
        _ => Err(format!("the parity route has incomplete availability at {path}").into()),
    }
}

#[derive(Clone, Copy)]
pub(super) struct DigestedRows<'scope>(&'scope str, &'scope DigestedTable);

impl<'de> DeserializeSeed<'de> for DigestedRows<'_> {
    type Value = Value;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Value, D::Error> {
        deserializer.deserialize_seq(self)
    }
}

impl<'de> Visitor<'de> for DigestedRows<'_> {
    type Value = Value;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a complete ordered Catalog row array")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Value, A::Error> {
        let Self(path, table) = self;
        for (index, (slot, (_, expected))) in table.rows.iter().enumerate() {
            let row = sequence.next_element()?.ok_or_else(|| {
                A::Error::custom(format!("missing Catalog row at {path}/rows/{index}"))
            })?;
            if fingerprint(row).map_err(A::Error::custom)? != *expected {
                return Err(A::Error::custom(format!(
                    "Catalog and SDK row fingerprints differ at {path}/rows/{index} (expected physical slot {slot:?})",
                )));
            }
        }
        if sequence.next_element::<IgnoredAny>()?.is_some() {
            return Err(A::Error::custom(format!(
                "extra Catalog row at {path}/rows/{}",
                table.rows.len()
            )));
        }
        Ok(Value::from(table.rows.len()))
    }
}

#[derive(Clone, Copy)]
pub(super) struct DigestedOrigins<'scope>(&'scope HashMap<String, WireDigest>);

impl<'de> DeserializeSeed<'de> for DigestedOrigins<'_> {
    type Value = Value;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Value, D::Error> {
        deserializer.deserialize_seq(self)
    }
}

impl<'de> Visitor<'de> for DigestedOrigins<'_> {
    type Value = Value;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a Catalog provenance row array")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Value, A::Error> {
        let includes = |id: &str| self.0.contains_key(id);
        let mut compared = HashSet::new();
        while let Some(row) = sequence.next_element_seed(SelectOrigin(&includes))? {
            let Some((id, row)) = row else { continue };
            let (key, expected) = self
                .0
                .get_key_value(&id)
                .ok_or_else(|| A::Error::custom("unexpected selected provenance identifier"))?;
            if !compared.insert(key) {
                return Err(A::Error::custom(
                    "duplicate selected Catalog provenance identifier",
                ));
            }
            if fingerprint(row).map_err(A::Error::custom)? != *expected {
                return Err(A::Error::custom(
                    "Catalog and SDK provenance fingerprints differ",
                ));
            }
        }
        if compared.len() != self.0.len() {
            return Err(A::Error::custom(
                "missing selected Catalog provenance identifier",
            ));
        }
        Ok(Value::from(compared.len()))
    }
}

#[cfg(test)]
#[path = "real_archive_catalog_digest_parity_tests.rs"]
mod tests;
