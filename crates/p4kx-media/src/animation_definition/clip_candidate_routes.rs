// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, HashSet};

use super::super::sub_database_components::Work;
use super::{Alias, ArcKind, RouteArc};
use crate::input_budget::InputBudget;
use crate::origin_capture::OriginCapture;
use crate::{MediaError, MediaWriter, origin_error};

pub(super) struct Routes<'facts> {
    readable: &'facts HashSet<usize>,
    outgoing: HashMap<(ArcKind, usize), Vec<&'facts RouteArc>>,
}

impl<'facts> Routes<'facts> {
    pub(super) fn new(
        readable: &'facts HashSet<usize>,
        arcs: &'facts [RouteArc],
        work: &Work<'_, '_>,
    ) -> Result<Self, MediaError> {
        let mut outgoing = HashMap::<_, Vec<_>>::new();
        for arc in arcs {
            work.charge()?;
            if !readable.contains(&arc.from) || !readable.contains(&arc.to) {
                continue;
            }
            outgoing
                .try_reserve(1)
                .map_err(|cause| work.allocation(&cause))?;
            let targets = outgoing.entry((arc.kind, arc.from)).or_default();
            targets
                .try_reserve(1)
                .map_err(|cause| work.allocation(&cause))?;
            targets.push(arc);
        }
        Ok(Self { readable, outgoing })
    }

    pub(super) fn walk(
        &self,
        kind: ArcKind,
        seeds: impl IntoIterator<Item = (usize, Seed<'facts>)>,
        work: &Work<'_, '_>,
    ) -> Result<Walk<'facts>, MediaError> {
        let mut walk = Walk::default();
        for (entry, seed) in seeds {
            work.charge()?;
            if self.readable.contains(&entry) {
                walk.admit(entry, Predecessor::Seed(seed), work)?;
            }
        }
        let mut next = 0;
        while let Some(&entry) = walk.entries.get(next) {
            work.charge()?;
            if let Some(outgoing) = self.outgoing.get(&(kind, entry)) {
                for &arc in outgoing {
                    work.charge()?;
                    walk.admit(arc.to, Predecessor::Arc(arc), work)?;
                }
            }
            next += 1;
        }
        Ok(walk)
    }
}

#[derive(Clone, Copy)]
pub(super) enum Seed<'facts> {
    Root,
    Space(&'facts Alias),
}

enum Predecessor<'facts> {
    Seed(Seed<'facts>),
    Arc(&'facts RouteArc),
}

#[derive(Default)]
pub(super) struct Walk<'facts> {
    entries: Vec<usize>,
    predecessors: HashMap<usize, Predecessor<'facts>>,
}

impl<'facts> Walk<'facts> {
    pub(super) fn entries(&self) -> &[usize] {
        &self.entries
    }

    pub(super) fn include(
        &self,
        mut entry: usize,
        proof: &mut OriginCapture,
        budget: &InputBudget,
        writer: &MediaWriter<'_>,
    ) -> Result<Seed<'facts>, MediaError> {
        let work = Work::new(budget, writer);
        loop {
            work.charge()?;
            match self
                .predecessors
                .get(&entry)
                .ok_or_else(origin_error::counter)?
            {
                Predecessor::Seed(seed) => return Ok(*seed),
                Predecessor::Arc(arc) => {
                    proof.include(&arc.proof, writer)?;
                    entry = arc.from;
                }
            }
        }
    }

    fn admit(
        &mut self,
        entry: usize,
        predecessor: Predecessor<'facts>,
        work: &Work<'_, '_>,
    ) -> Result<(), MediaError> {
        if self.predecessors.contains_key(&entry) {
            return Ok(());
        }
        self.predecessors
            .try_reserve(1)
            .map_err(|cause| work.allocation(&cause))?;
        self.entries
            .try_reserve(1)
            .map_err(|cause| work.allocation(&cause))?;
        self.predecessors.insert(entry, predecessor);
        self.entries.push(entry);
        Ok(())
    }
}
