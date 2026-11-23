// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, HashSet};
use std::ops::Range;

use crate::fields::layouts::SharedLayouts;
use crate::fields::number::Extremes;
use crate::fields::observation::{FieldStatisticsInput, FieldStatisticsObservation, Observe};
use crate::fields::observation_route::Routes;
use crate::fields::seen::Seen;
use crate::fields::trie::{ROOT, Trie};
use crate::fields::walk_frame::Site;
use crate::fields::{FieldKind, FieldStatisticsError};
use crate::fields::{
    FieldStatisticsDistinctUse, FieldStatisticsGap, FieldStatisticsOperand,
    FieldStatisticsScalarRead, FieldStatisticsUse, FieldStatisticsValue,
};
use crate::source_layout_index;
use crate::{DataForge, DataForgeError, LayoutSource};

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

/// The original input and value state consulted for a counted occurrence.
struct PendingProof {
    input: FieldStatisticsInput,
    seen: Seen,
    text: Option<Range<u64>>,
    scalar_read: FieldStatisticsScalarRead,
    route: Option<usize>,
}

impl PendingProof {
    fn operand(&self) -> FieldStatisticsOperand {
        FieldStatisticsOperand::new(self.input.clone(), self.text.clone(), self.scalar_read)
    }
}

/// The statistics of the Records one fold part has walked, with the scratch space of the walk.
pub(crate) struct Part<'s, 'f> {
    pub(crate) trie: Trie<'f>,
    pub(super) layouts: &'s SharedLayouts<'f>,
    pub(super) types: HashMap<(u32, u32), u32>,
    pub(super) visited: HashSet<(u32, u32)>,
    pub(super) observer: Option<Observe<'s>>,
    pub(super) routes: Routes,
    capture_comparisons: bool,
    pending: Vec<Pending>,
    proofs: Vec<Option<PendingProof>>,
    pub(super) skipped: u64,
    pub(super) record: u32,
    pub(crate) examined: u64,
    pub(crate) unreadable_records: u64,
    pub(crate) unreadable_values: u64,
}

impl<'s, 'f> Part<'s, 'f> {
    pub(super) fn new(
        layouts: &'s SharedLayouts<'f>,
        observer: Option<Observe<'s>>,
        capture_comparisons: bool,
    ) -> Self {
        Self {
            trie: Trie::new(),
            layouts,
            types: HashMap::new(),
            visited: HashSet::new(),
            observer,
            routes: Routes::new(),
            capture_comparisons,
            pending: Vec::new(),
            proofs: Vec::new(),
            skipped: 0,
            record: NONE,
            examined: 0,
            unreadable_records: 0,
            unreadable_values: 0,
        }
    }

    /// Adds two parts, dropping the scratch space of both.
    pub(crate) fn merge(self, other: Self) -> Result<Self, FieldStatisticsError> {
        let mut merged = Self::new(self.layouts, self.observer, self.capture_comparisons);
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
            self.unreadable_record(index, FieldStatisticsGap::MissingRecord);
            return Ok(());
        };
        let instance = u32::from(def.instance_index);
        let layouts = self.layouts;
        let Some(bytes) = forge.instance(def.struct_index, instance) else {
            self.unreadable_record(index, FieldStatisticsGap::MissingInstance);
            return Ok(());
        };
        let layout = match layouts.get(forge, def.struct_index) {
            Ok(layout) => layout,
            Err(error) if source_layout_index::is_operational(&error) => {
                return Err(FieldStatisticsError::Read { source: error });
            }
            Err(_) => {
                self.unreadable_record(index, FieldStatisticsGap::UnreadableLayout);
                return Ok(());
            }
        };
        self.observe(
            ROOT,
            FieldStatisticsInput::Record(index),
            FieldStatisticsUse::Record {
                structure: def.struct_index,
                instance,
            },
        );
        self.record = index;
        self.routes.reset();
        self.skipped = 0;
        self.pending.clear();
        self.proofs.clear();
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

    fn unreadable_record(&mut self, record: u32, gap: FieldStatisticsGap) {
        self.unreadable_records += 1;
        if self.capture_comparisons
            && gap != FieldStatisticsGap::MissingRecord
            && let Some(root) = self.trie.nodes.first_mut()
        {
            root.gaps.root_unreadable();
        }
        self.observe(
            ROOT,
            FieldStatisticsInput::Record(record),
            FieldStatisticsUse::Skipped(gap),
        );
    }

    fn observe(&self, node: u32, input: FieldStatisticsInput, usage: FieldStatisticsUse) {
        if let Some(observer) = self.observer {
            observer(FieldStatisticsObservation::new(
                &self.trie,
                node,
                input,
                usage,
                self.routes.at(self.routes.position()),
            ));
        }
    }

    pub(super) fn observe_layout(&self, node: u32, layout: LayoutSource) {
        self.observe(
            node,
            FieldStatisticsInput::Record(self.record),
            FieldStatisticsUse::Layout(layout),
        );
    }

    pub(super) fn consulted(&self, site: &Site) {
        self.observe_site(site, FieldStatisticsUse::Consulted);
    }

    pub(super) fn observe_site(&self, site: &Site, usage: FieldStatisticsUse) {
        if let Some(input) = &site.input {
            self.observe(site.node, input.clone(), usage);
        }
    }

    pub(super) fn skip(&mut self, site: &Site, gap: FieldStatisticsGap) {
        self.skipped += 1;
        if self.capture_comparisons
            && let Some(node) = self.trie.nodes.get_mut(site.node as usize)
        {
            node.gaps.observe(gap, site.input.as_ref());
        }
        self.observe_site(site, FieldStatisticsUse::Skipped(gap));
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
    pub(super) fn leaf(
        &mut self,
        site: &Site,
        kind: FieldKind,
        seen: Seen,
        empty: bool,
        text: Option<Range<u64>>,
    ) {
        self.leaf_with_sources(
            site,
            kind,
            seen,
            empty,
            text,
            FieldStatisticsScalarRead::default(),
        );
    }

    pub(super) fn leaf_with_sources(
        &mut self,
        site: &Site,
        kind: FieldKind,
        seen: Seen,
        empty: bool,
        text: Option<Range<u64>>,
        scalar_read: FieldStatisticsScalarRead,
    ) {
        if self.observer.is_some() {
            self.proofs
                .push(site.input.as_ref().map(|input| PendingProof {
                    input: input.clone(),
                    seen,
                    text,
                    scalar_read,
                    route: self.routes.position(),
                }));
        }
        let seen = if matches!(seen, Seen::Unreadable) {
            self.skip(site, FieldStatisticsGap::UnreadableValue);
            Seen::Empty
        } else {
            seen
        };
        self.pending.push(Pending {
            node: site.node,
            kind,
            seen,
            empty,
        });
    }

    fn commit(&mut self) {
        let record = self.record;
        let mut proofs = self.proofs.drain(..);
        for leaf in self.pending.drain(..) {
            let proof = proofs.next().flatten();
            let operand = proof
                .as_ref()
                .filter(|_| self.capture_comparisons)
                .map(PendingProof::operand);
            let Some(entry) = self.trie.nodes.get_mut(leaf.node as usize) else {
                continue;
            };
            entry.count(record, leaf.kind, leaf.empty);
            let distinct = if let Seen::Value {
                number,
                fingerprint,
            } = leaf.seen
            {
                if let Some(number) = number {
                    match &mut entry.extremes {
                        Some(extremes) => extremes.add(number, operand.as_ref()),
                        unset => {
                            *unset = Some(Box::new(Extremes::of(
                                number,
                                self.capture_comparisons,
                                operand.as_ref(),
                            )));
                        }
                    }
                }
                entry
                    .distinct
                    .get_or_insert_with(|| {
                        Box::new(super::number::Distinct::new(self.capture_comparisons))
                    })
                    .add(fingerprint, operand.as_ref())
            } else {
                FieldStatisticsDistinctUse::NoCandidate
            };
            if let Some(observer) = self.observer
                && let Some(proof) = proof
            {
                observer(FieldStatisticsObservation::new(
                    &self.trie,
                    leaf.node,
                    proof.input,
                    FieldStatisticsUse::Counted {
                        kind: leaf.kind,
                        value: FieldStatisticsValue::from_read(
                            proof.seen, leaf.empty, proof.text, distinct,
                        ),
                        scalar_read: proof.scalar_read,
                    },
                    self.routes.at(proof.route),
                ));
            }
        }
        drop(proofs);
        self.unreadable_values += self.skipped;
        self.skipped = 0;
        self.release();
    }

    fn release(&mut self) {
        if self.pending.capacity() > KEPT_PENDING {
            self.pending = Vec::new();
            self.proofs = Vec::new();
        }
    }
}
