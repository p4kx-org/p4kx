// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};

use p4kx_extraction::{
    Archive, Build, DataForge, DataForgeOrigin, InputScope, PhysicalSource, PhysicalSourceKind,
    PhysicalSourceRef, SourceEncryptionState, SourceInput, SourceInventory, SourceUseDescription,
    SourceUseRole, SourceUses,
};
use serde_json::Value;

use crate::catalog_queries::{CatalogRecordCoverage, DatasetAvailability};
use crate::record_header_field::RecordHeaderField;

use super::receipt::Report;
use super::selection::{Census, Family};
use super::{TestResult, scope};

#[path = "../tests/real_archive/contract_sdk_physical_facts.rs"]
mod physical_facts;

pub(super) struct Proof {
    inventory: SourceInventory,
    leaf: PhysicalSourceRef,
}

pub(super) struct Context<'query> {
    pub(super) archive: &'query Archive,
    pub(super) forge: &'query DataForge<'query>,
    pub(super) proof: &'query Proof,
    pub(super) family: Family,
    pub(super) census: &'query Census,
}

#[derive(Clone, Copy)]
pub(super) struct Evidence<'query> {
    pub(super) build: Option<&'query Build>,
    pub(super) availability: DatasetAvailability,
    pub(super) inventory: &'query SourceInventory,
    pub(super) coverage: &'query CatalogRecordCoverage,
    pub(super) dependencies: Option<&'query CatalogRecordCoverage>,
}

impl Proof {
    pub(super) fn read(origin: &DataForgeOrigin) -> TestResult<Self> {
        let DataForgeOrigin::Archive(origin) = origin else {
            return Err("expected original Archive-bound DataForge evidence".into());
        };
        let uses = SourceUses::new();
        let scope = uses
            .observe(SourceInput::Archive(origin), InputScope::Payload)
            .map_err(|_| "original DataForge evidence capture failed")?;
        Ok(Self {
            leaf: scope.source_ref().clone(),
            inventory: uses
                .freeze()
                .map_err(|_| "original evidence freeze failed")?,
        })
    }

    fn mapped_leaf<'actual>(
        &self,
        inventory: &'actual SourceInventory,
    ) -> TestResult<&'actual PhysicalSource> {
        let mut mapping = BTreeMap::new();
        for expected in self.inventory.iter_sources() {
            let mut matches = inventory
                .iter_sources()
                .filter(|actual| physical_facts::same(expected, actual));
            let actual = matches
                .next()
                .ok_or("original physical source facts missing")?;
            if matches.next().is_some() {
                return Err("original physical source facts are ambiguous".into());
            }
            mapping.insert(expected.id(), actual);
        }
        for expected in self.inventory.iter_sources() {
            let actual = mapping
                .get(expected.id())
                .ok_or("original source mapping missing")?;
            for (parent, mapped) in expected.ancestors().iter().zip(actual.ancestors()) {
                if mapping.get(parent).map(|source| source.id()) != Some(mapped) {
                    return Err("original ancestor identity or order differs".into());
                }
            }
        }
        mapping
            .get(&self.leaf)
            .copied()
            .ok_or_else(|| "original DCB source missing".into())
    }
}

impl Context<'_> {
    pub(super) fn check(
        &self,
        evidence: Evidence<'_>,
        tables: &[Value],
        text_context: &[Value],
    ) -> TestResult<Report> {
        self.summary(&evidence)?;
        let selected = self.census.owners.first().copied();
        let leaf = self.proof.mapped_leaf(evidence.inventory)?;
        let graph = Graph::read(evidence.inventory)?;
        graph.physical_closure(self.archive, self.proof)?;
        scope::validate(self, evidence.inventory, leaf.id())?;
        let mut cells = 0;
        let mut row_counts = Vec::new();
        for (table_index, table) in tables.iter().enumerate() {
            let rows = table.as_array().ok_or("SDK row table must be an array")?;
            if rows.len() != usize::from(selected.is_some()) {
                return Err("physical selection did not return exactly its original owner".into());
            }
            row_counts.push(rows.len());
            for row in rows {
                self.identity(
                    row,
                    table_index == 0 && self.family != Family::Parents,
                    &graph,
                )?;
                scope::row(self, row)?;
            }
            cells += graph.walk(table)?;
        }
        for context in text_context {
            cells += graph.walk(context)?;
        }
        for source in evidence.inventory.iter_sources() {
            cells += graph.walk(
                &serde_json::to_value(source.kind())
                    .map_err(|_| "physical metadata inspection failed")?,
            )?;
        }
        Ok(Report::new(
            self.family,
            self.census,
            evidence,
            row_counts,
            cells,
        ))
    }

    fn summary(&self, evidence: &Evidence<'_>) -> TestResult {
        if evidence.build != self.archive.build() {
            return Err("SDK result Build differs from the opened Archive".into());
        }
        if std::mem::discriminant(&evidence.availability)
            != std::mem::discriminant(&self.census.availability())
        {
            return Err("SDK availability differs from the original declaration census".into());
        }
        let selected = self.census.owners.first().copied();
        let skipped = if selected.is_some() {
            0
        } else {
            self.census.leftovers
        };
        coverage(evidence.coverage, skipped)?;
        if let Some(dependencies) = evidence.dependencies {
            let skipped = if self.family == Family::Parents && selected.is_some() {
                if self.census.possible_unreadable > 0 {
                    return Err(
                        "unknown possible group owner prevents complete parent survey".into(),
                    );
                }
                self.census.leftovers
            } else {
                0
            };
            coverage(dependencies, skipped)?;
        }
        Ok(())
    }

    fn identity(&self, row: &Value, has_header: bool, graph: &Graph<'_>) -> TestResult {
        let index = *self
            .census
            .owners
            .first()
            .ok_or("original owner required")?;
        let record = self
            .forge
            .record(index)
            .ok_or("original physical owner missing")?;
        if row.pointer("/recordIndex/value").and_then(Value::as_u64) != Some(u64::from(index))
            || row.pointer("/guid/value").and_then(Value::as_str)
                != Some(record.guid().to_string().as_str())
        {
            return Err("SDK row differs from the original physical owner identity".into());
        }
        graph.header(row, "recordIndex", index, RecordHeaderField::RecordIndex)?;
        graph.header(row, "guid", index, RecordHeaderField::Guid)?;
        if has_header {
            let source_type = record
                .type_name()
                .map_err(|_| "original owner type unavailable")?;
            if row.pointer("/sourceType/value").and_then(Value::as_str) != Some(source_type) {
                return Err("SDK row differs from the original declared source type".into());
            }
            graph.header(row, "sourceType", index, RecordHeaderField::SourceType)?;
        }
        Ok(())
    }
}

pub(super) fn coverage(value: &CatalogRecordCoverage, skipped: usize) -> TestResult {
    if !value.unreadable_headers.is_empty()
        || !value.unreadable_bodies.is_empty()
        || !value.incomplete_fields.is_empty()
        || value.skipped_leftovers != skipped
    {
        return Err(
            "SDK coverage does not establish the demanded selected/dependency scope".into(),
        );
    }
    Ok(())
}

struct Graph<'inventory> {
    sources: BTreeMap<&'inventory str, &'inventory PhysicalSource>,
    uses: BTreeMap<&'inventory str, &'inventory SourceUseDescription>,
}

impl<'inventory> Graph<'inventory> {
    fn read(inventory: &'inventory SourceInventory) -> TestResult<Self> {
        let sources: BTreeMap<_, _> = inventory
            .iter_sources()
            .map(|s| (s.id().as_str(), s))
            .collect();
        let uses: BTreeMap<_, _> = inventory
            .iter_uses()
            .map(|u| (u.id().as_str(), u))
            .collect();
        if sources.len() != inventory.source_count() || uses.len() != inventory.use_count() {
            return Err("SDK source/use identifiers must be unique".into());
        }
        for usage in uses.values() {
            if !sources.contains_key(usage.source().as_str()) {
                return Err("SDK source-use points outside the physical inventory".into());
            }
        }
        Ok(Self { sources, uses })
    }

    fn physical_closure(&self, archive: &Archive, proof: &Proof) -> TestResult {
        for source in self.sources.values() {
            for (position, ancestor) in source.ancestors().iter().enumerate() {
                let parent = self
                    .sources
                    .get(ancestor.as_str())
                    .ok_or("missing physical ancestor")?;
                if parent.ancestors() != &source.ancestors()[..position] {
                    return Err("physical ancestor chain is not closed and ordered".into());
                }
            }
            match source.kind() {
                PhysicalSourceKind::ArchiveEntry(_)
                    if proof
                        .inventory
                        .iter_sources()
                        .any(|expected| physical_facts::same(expected, source)) => {}
                PhysicalSourceKind::ArchiveDirectory(directory)
                    if usize::try_from(directory.admitted_entry_count().get()).ok()
                        == Some(archive.entries().len()) => {}
                _ => {
                    return Err(
                        "SDK source escaped DCB and explicit Archive-directory metadata".into(),
                    );
                }
            }
        }
        Ok(())
    }

    fn header(&self, row: &Value, name: &str, index: u32, field: RecordHeaderField) -> TestResult {
        let cell = row.get(name).ok_or("physical header cell missing")?;
        let refs = cell
            .get("sourceUsesRefs")
            .and_then(Value::as_array)
            .ok_or("physical header source references missing")?;
        if !refs.iter().any(|reference| {
            reference
                .as_str()
                .and_then(|id| self.uses.get(id))
                .is_some_and(|usage| usage.location() == &field.location(index))
        }) {
            return Err("SDK header cell lost its exact original header coordinate".into());
        }
        Ok(())
    }

    fn walk(&self, value: &Value) -> TestResult<usize> {
        match value {
            Value::Array(values) => values
                .iter()
                .try_fold(0, |count, value| Ok(count + self.walk(value)?)),
            Value::Object(fields) => {
                let mut count = 0;
                if fields.contains_key("sourceUsesRefs") {
                    self.cell(value)?;
                    count += 1;
                }
                for value in fields.values() {
                    count += self.walk(value)?;
                }
                Ok(count)
            }
            _ => Ok(0),
        }
    }

    fn cell(&self, cell: &Value) -> TestResult {
        if cell.get("sourceSetComplete").and_then(Value::as_bool) != Some(true) {
            return Err("SDK cell does not retain a complete original contribution set".into());
        }
        let references = cell
            .get("sourceUsesRefs")
            .and_then(Value::as_array)
            .ok_or("SDK cell source-use list missing")?;
        let mut seen = BTreeSet::new();
        let mut encrypted = false;
        let mut unknown = false;
        for reference in references {
            let id = reference
                .as_str()
                .ok_or("SDK cell source-use reference must be text")?;
            if !seen.insert(id) {
                return Err("SDK cell repeats an immediate source-use reference".into());
            }
            let usage = self
                .uses
                .get(id)
                .ok_or("SDK cell source-use reference is unresolved")?;
            if !matches!(
                usage.role(),
                SourceUseRole::Payload | SourceUseRole::Selector | SourceUseRole::Conversion
            ) {
                return Err(
                    "SDK cell cites an attempted or untouched target as a contributor".into(),
                );
            }
            encrypted |= usage.effective_encryption() == SourceEncryptionState::Encrypted;
            unknown |= usage.effective_encryption() == SourceEncryptionState::Unknown;
        }
        let expected = if encrypted {
            Some(true)
        } else if unknown {
            None
        } else {
            Some(false)
        };
        let marker = cell
            .get("containsEncryptedSource")
            .ok_or("SDK cell encryption marker missing")?;
        if !(marker.is_null() || marker.is_boolean()) || marker.as_bool() != expected {
            return Err("SDK cell encryption marker differs from its actual contributors".into());
        }
        Ok(())
    }
}
