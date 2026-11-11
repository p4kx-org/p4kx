// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::ops::Range;

use crate::tree_cursor::Cursor;
use crate::{DataForge, Field, Guid, Record, Step, TreeBuilder, Value, ValueError};

use super::UNRESOLVED;
use super::observed::{Evidence, ReferenceIndexError};

/// The Records each parallel task reads.
pub(super) const RECORDS_PER_TASK: u32 = 512;

/// The References of a run of Records in the order of the Records, with field shapes and array
/// indices stored in shared buffers.
#[derive(Debug, Default)]
pub(super) struct Chunk {
    pub(super) sources: Vec<u32>,
    pub(super) targets: Vec<u32>,
    pub(super) shapes: Vec<usize>,
    pub(super) index_starts: Vec<usize>,
    pub(super) indices: Vec<u32>,
    pub(super) unresolved: Vec<(usize, [u8; 16])>,
    pub(super) shape_ids: HashMap<Box<str>, usize>,
    pub(super) unreadable: u32,
    pub(super) truncated: bool,
    pub(super) evidence: Option<Evidence>,
}

impl Chunk {
    fn shape(&mut self, text: &str) -> usize {
        if let Some(id) = self.shape_ids.get(text) {
            return *id;
        }
        let id = self.shape_ids.len();
        self.shape_ids.insert(text.into(), id);
        id
    }
}

/// Reads References in Record order until the first omission or cancellation.
pub(super) fn collect<'f, const OBSERVED: bool>(
    forge: &'f DataForge<'f>,
    builder: &mut TreeBuilder<'f>,
    range: Range<u32>,
    limit: usize,
    stop: &(dyn Fn() -> bool + Sync),
) -> Chunk {
    let mut chunk = Chunk {
        evidence: OBSERVED.then(Evidence::default),
        ..Chunk::default()
    };
    let mut walker = Walker::<OBSERVED> {
        source: 0,
        limit,
        path: String::new(),
        indices: Vec::new(),
        chunk: &mut chunk,
    };
    for index in range {
        if walker.chunk.truncated
            || walker.chunk.evidence.as_ref().is_some_and(Evidence::failed)
            || stop()
        {
            break;
        }
        let Some(record) = forge.record(index) else {
            if let Some(evidence) = &mut walker.chunk.evidence {
                evidence.fail(ReferenceIndexError::Inconsistent);
            }
            continue;
        };
        if !walker.record(builder, &record) {
            walker.chunk.unreadable += 1;
        }
    }
    chunk
}

struct Walker<'w, const OBSERVED: bool> {
    source: u32,
    limit: usize,
    path: String,
    indices: Vec<u32>,
    chunk: &'w mut Chunk,
}

impl<const OBSERVED: bool> Walker<'_, OBSERVED> {
    fn record<'f>(&mut self, builder: &mut TreeBuilder<'f>, record: &Record<'f>) -> bool {
        let tree = match builder.build(record) {
            Ok(tree) => tree,
            Err(error) => {
                if let Some(evidence) = &mut self.chunk.evidence {
                    evidence.tree_error(record.index(), error);
                }
                return false;
            }
        };
        self.source = record.index();
        self.path.clear();
        self.indices.clear();
        self.fields(tree.fields());
        true
    }

    fn fields(&mut self, fields: &[Field<'_>]) {
        let mut cursor = Cursor::new(fields);
        let mut ordinal = 0_u64;
        while let Some(visit) = cursor.next() {
            let current = ordinal;
            if OBSERVED {
                let Some(next) = ordinal.checked_add(1) else {
                    if let Some(evidence) = &mut self.chunk.evidence {
                        evidence.fail(ReferenceIndexError::Inconsistent);
                    }
                    break;
                };
                ordinal = next;
                if let Value::Error(error) = visit.value
                    && let Some(evidence) = &mut self.chunk.evidence
                {
                    evidence.value_error(self.source, current, visit.data_type, *error);
                    if evidence.failed() {
                        break;
                    }
                }
            }
            let (target, guid) = match visit.value {
                Value::Reference(reference) if reference.record_index() != self.source => {
                    (reference.record_index(), reference.guid())
                }
                Value::Error(ValueError::DanglingReference { guid }) => (UNRESOLVED, *guid),
                _ => continue,
            };
            if self.chunk.sources.len() == self.limit {
                self.chunk.truncated = true;
                break;
            }
            if let Some(evidence) = &mut self.chunk.evidence
                && let Err(error) = evidence.retain(current)
            {
                evidence.fail(error);
                break;
            }
            self.set_path(visit.steps);
            self.push(target, guid);
        }
    }

    fn set_path(&mut self, steps: &[Step<'_>]) {
        self.path.clear();
        self.indices.clear();
        for step in steps {
            match step {
                Step::Field(name) => {
                    if !self.path.is_empty() {
                        self.path.push('.');
                    }
                    self.path.push_str(name);
                }
                Step::Element(index) => {
                    self.path.push_str("[]");
                    self.indices.push(*index);
                }
            }
        }
    }

    fn push(&mut self, target: u32, guid: Guid) {
        let shape = self.chunk.shape(&self.path);
        let start = self.chunk.indices.len();
        let entry = self.chunk.sources.len();
        self.chunk.sources.push(self.source);
        self.chunk.targets.push(target);
        self.chunk.shapes.push(shape);
        self.chunk.index_starts.push(start);
        self.chunk.indices.extend_from_slice(&self.indices);
        if target == UNRESOLVED {
            self.chunk.unresolved.push((entry, guid.stored()));
        }
    }
}
