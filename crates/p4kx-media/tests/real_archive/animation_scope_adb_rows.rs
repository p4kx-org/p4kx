// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, BTreeSet};

use p4kx_media::{Identity, MediaRecord};
use serde_json::Value;

use super::{TestResult, require};

const EDGE: &str = "animation-definition-sub-database-edge";
const CYCLE: &str = "animation-definition-sub-database-cycle";
const FRONTIER: &str = "animation-definition-sub-database-target-frontier";

#[derive(Default)]
pub(super) struct Graph {
    roots: BTreeSet<usize>,
    edges: BTreeMap<(usize, usize), usize>,
    frontiers: BTreeMap<usize, usize>,
}

impl Graph {
    pub(super) fn document(&mut self, entry: usize) {
        self.roots.insert(entry);
    }

    pub(super) fn branch(&mut self, source: usize, target: usize) {
        *self.edges.entry((source, target)).or_default() += 1;
    }

    pub(super) fn prepare(&mut self) {
        self.edges.retain(|&(source, target), count| {
            if self.roots.contains(&target) {
                return true;
            }
            *self.frontiers.entry(source).or_default() += *count;
            false
        });
    }

    pub(super) fn observe(&mut self, row: &MediaRecord) -> TestResult<bool> {
        match row.subresource.namespace.as_str() {
            EDGE => self.edge(row)?,
            CYCLE => self.cycle(row)?,
            FRONTIER => self.frontier(row)?,
            _ => return Ok(false),
        }
        require(
            matches!(row.source.identity, Identity::Root),
            "archive-wide graph identity",
        )?;
        Ok(true)
    }

    fn edge(&mut self, row: &MediaRecord) -> TestResult {
        let source = number(row, "/source_entry_index")?;
        let target = number(row, "/target_entry_index")?;
        let element = number(row, "/source_element_index")?;
        let declaration = number(row, "/source_declaration_ordinal")?;
        require(
            row.subresource.indices == [source, element, declaration],
            "SubADB edge identity includes its physical source declaration",
        )?;
        consume(
            &mut self.edges,
            (usize::try_from(source)?, usize::try_from(target)?),
        )
    }

    fn cycle(&self, row: &MediaRecord) -> TestResult {
        let representative = number(row, "/representative_entry_index")?;
        require(
            row.subresource.indices == [representative]
                && self.roots.contains(&usize::try_from(representative)?)
                && number(row, "/member_count")? > 0
                && number(row, "/edge_count")? > 0,
            "cycle summary names a selected Mannequin document",
        )
    }

    fn frontier(&mut self, row: &MediaRecord) -> TestResult {
        let [source, element, declaration] = row.subresource.indices.as_slice() else {
            return Err("SubADB target frontier uses its physical source declaration".into());
        };
        require(
            *element == number(row, "/source_element_index")?
                && *declaration == number(row, "/source_declaration_ordinal")?
                && row.data.pointer("/payload/reason").and_then(Value::as_str)
                    == Some("unsupported-target-root"),
            "verified target has an unsupported Mannequin root",
        )?;
        consume(&mut self.frontiers, usize::try_from(*source)?)
    }

    pub(super) fn verify(&self) -> TestResult {
        require(
            self.edges.is_empty() && self.frontiers.is_empty(),
            "every verified physical SubADB edge retained",
        )
    }
}

fn consume<K: Ord>(counts: &mut BTreeMap<K, usize>, key: K) -> TestResult {
    let Entry::Occupied(mut original) = counts.entry(key) else {
        return Err("SubADB graph row has no matching original physical edge".into());
    };
    if *original.get() == 1 {
        original.remove();
    } else {
        *original.get_mut() -= 1;
    }
    Ok(())
}

fn number(row: &MediaRecord, field: &str) -> TestResult<u64> {
    Ok(row
        .data
        .get("payload")
        .and_then(|payload| payload.pointer(field))
        .and_then(Value::as_str)
        .ok_or("decimal graph field")?
        .parse()?)
}
