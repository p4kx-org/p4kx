// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, HashSet};

use crate::fields::layouts::SharedLayouts;
use crate::fields::number::Extremes;
use crate::fields::seen::Seen;
use crate::fields::trie::{ROOT, Trie};
use crate::fields::{FieldKind, FieldStatisticsError};
use crate::source_layout_index;
use crate::{DataForge, DataForgeError};

pub(super) const NONE: u32 = u32::MAX;

const KEPT_PENDING: usize = 1 << 16;

/// Why the walk of one Record stops.
pub(super) enum Stop {
    Paths(FieldStatisticsError),
    Read(DataForgeError),
}

impl From<FieldStatisticsError> for Stop {
    fn from(error: FieldStatisticsError) -> Self {
        Self::Paths(error)
    }
}

impl From<DataForgeError> for Stop {
    fn from(error: DataForgeError) -> Self {
        Self::Read(error)
    }
}

pub(super) type Walk = Result<(), Stop>;

/// A value found in the Record being walked, counted once the Record is known to be readable.
#[derive(Debug)]
struct Pending {
    node: u32,
    kind: FieldKind,
    seen: Seen,
    empty: bool,
}

/// The statistics of the Records one fold part has walked, with the scratch space of the walk.
#[derive(Debug)]
pub(crate) struct Part<'s, 'f> {
    pub(crate) trie: Trie<'f>,
    pub(super) layouts: &'s SharedLayouts<'f>,
    pub(super) types: HashMap<(u32, u32), u32>,
    pub(super) visited: HashSet<(u32, u32)>,
    pending: Vec<Pending>,
    pub(super) skipped: u64,
    record: u32,
    pub(crate) examined: u64,
    pub(crate) unreadable_records: u64,
    pub(crate) unreadable_values: u64,
}

impl<'s, 'f> Part<'s, 'f> {
    pub(crate) fn new(layouts: &'s SharedLayouts<'f>) -> Self {
        Self {
            trie: Trie::new(),
            layouts,
            types: HashMap::new(),
            visited: HashSet::new(),
            pending: Vec::new(),
            skipped: 0,
            record: NONE,
            examined: 0,
            unreadable_records: 0,
            unreadable_values: 0,
        }
    }

    /// Adds two parts, dropping the scratch space of both.
    pub(crate) fn merge(self, other: Self) -> Result<Self, FieldStatisticsError> {
        let mut merged = Self::new(self.layouts);
        merged.examined = self.examined + other.examined;
        merged.unreadable_records = self.unreadable_records + other.unreadable_records;
        merged.unreadable_values = self.unreadable_values + other.unreadable_values;
        merged.trie = self.trie.merge(other.trie)?;
        Ok(merged)
    }

    /// Walks the actual Record and atomically commits its readable field statistics.
    ///
    /// # Errors
    ///
    /// Returns [`FieldStatisticsError::TooManyPaths`] when the Record has more paths than may be
    /// kept.
    pub(crate) fn add(
        &mut self,
        forge: &'f DataForge<'f>,
        index: u32,
    ) -> Result<(), FieldStatisticsError> {
        self.examined += 1;
        let Some(def) = forge.record_def(index) else {
            self.unreadable_records += 1;
            return Ok(());
        };
        let instance = u32::from(def.instance_index);
        let layouts = self.layouts;
        let Some(bytes) = forge.instance(def.struct_index, instance) else {
            self.unreadable_records += 1;
            return Ok(());
        };
        let layout = match layouts.get(forge, def.struct_index) {
            Ok(layout) => layout,
            Err(error) if source_layout_index::is_operational(&error) => {
                return Err(FieldStatisticsError::Read { source: error });
            }
            Err(_) => {
                self.unreadable_records += 1;
                return Ok(());
            }
        };
        self.record = index;
        self.skipped = 0;
        self.pending.clear();
        self.visited.clear();
        self.visited.insert((def.struct_index, instance));
        match self.structure(forge, def.struct_index, &layout, bytes, ROOT, instance) {
            Ok(()) => {
                self.commit();
                Ok(())
            }
            Err(Stop::Paths(error)) => Err(error),
            Err(Stop::Read(source)) => Err(FieldStatisticsError::Read { source }),
        }
    }

    pub(super) fn field_node(&mut self, parent: u32, name: &'f str) -> Result<u32, Stop> {
        Ok(self.trie.child(parent, name)?)
    }
    pub(super) fn type_node(
        &mut self,
        parent: u32,
        struct_index: u32,
        name: &'f str,
    ) -> Result<u32, Stop> {
        if let Some(&known) = self.types.get(&(parent, struct_index)) {
            return Ok(known);
        }
        let id = self.trie.child(parent, name)?;
        self.types.insert((parent, struct_index), id);
        Ok(id)
    }

    /// Notes one value at `node`; `seen` says what it adds beyond its count. The value is counted
    /// when the Record has been walked to its end.
    pub(super) fn leaf(&mut self, node: u32, kind: FieldKind, seen: Seen, empty: bool) {
        let seen = if matches!(seen, Seen::Unreadable) {
            self.skipped += 1;
            Seen::Empty
        } else {
            seen
        };
        self.pending.push(Pending {
            node,
            kind,
            seen,
            empty,
        });
    }

    fn commit(&mut self) {
        let record = self.record;
        for leaf in self.pending.drain(..) {
            let Some(entry) = self.trie.nodes.get_mut(leaf.node as usize) else {
                continue;
            };
            entry.count(record, leaf.kind, leaf.empty);
            if let Seen::Value {
                number,
                fingerprint,
            } = leaf.seen
            {
                if let Some(number) = number {
                    match &mut entry.extremes {
                        Some(extremes) => extremes.add(number),
                        unset => *unset = Some(Box::new(Extremes::of(number))),
                    }
                }
                entry
                    .distinct
                    .get_or_insert_with(Box::default)
                    .add(fingerprint);
            }
        }
        self.unreadable_values += self.skipped;
        self.skipped = 0;
        self.release();
    }

    fn release(&mut self) {
        if self.pending.capacity() > KEPT_PENDING {
            self.pending = Vec::new();
        }
    }
}
