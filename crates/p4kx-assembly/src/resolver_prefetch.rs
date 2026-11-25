// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;
use std::sync::Arc;

use p4kx_dataforge::{Guid, Record, TreeBuilder};
use rayon::iter::{IntoParallelIterator, ParallelIterator};

use crate::resolver::SpecResult;
use crate::resolver_observation::{self, LoadoutReadEvent};
use crate::spec::{EntrySpec, LoadoutSpec};
use crate::{LoadoutResolver, MAX_LOADOUT_DEPTH};

/// The entries nested below a target whose spec is not read yet.
type Deferred = (Guid, Arc<[EntrySpec]>);

impl<'f> LoadoutResolver<'f> {
    /// Reads the specs of the Entities reachable from `root` through Record Loadouts and port
    /// defaults in parallel, level by level, as deep as a tree is resolved.
    pub(crate) fn prefetch(&mut self, root: Record<'f>) {
        let mut frontier = vec![root];
        let mut deferred = Vec::new();
        for depth in 0..=MAX_LOADOUT_DEPTH {
            if self.stopped() {
                break;
            }
            let (cached, missing): (Vec<_>, Vec<_>) = frontier
                .into_iter()
                .partition(|record| self.specs.contains_key(&record.index()));
            for record in &cached {
                if resolver_observation::notify(
                    self.observation.as_deref(),
                    LoadoutReadEvent::Reused { record },
                )
                .is_err()
                {
                    return;
                }
            }
            let observation = self.observation.clone();
            let forge = self.entities.forge();
            let forge_origin = self.forge_origin.as_ref();
            let record_cache = self.record_cache;
            let read: Vec<(Record<'f>, SpecResult)> = missing
                .into_par_iter()
                .map_init(TreeBuilder::new, |builder, record| {
                    let origin = forge_origin.filter(|_| record.is_from(forge));
                    (
                        record,
                        Arc::new(resolver_observation::read_spec(
                            builder,
                            &record,
                            observation.as_deref(),
                            origin,
                            record_cache,
                        )),
                    )
                })
                .collect();
            let mut level: Vec<SpecResult> = cached
                .iter()
                .filter_map(|record| self.specs.get(&record.index()).cloned())
                .collect();
            for (record, spec) in read {
                level.push(Arc::clone(&spec));
                self.specs.insert(record.index(), spec);
            }
            if self.stop_at_docked && depth > 0 {
                level.retain(|spec| spec.as_ref().as_ref().is_ok_and(|s| !s.docked_vehicle));
            }
            frontier = self.unread_targets(&level, &mut deferred);
            if frontier.is_empty() {
                break;
            }
        }
    }

    /// Returns the Records the specs name that are not read yet, each once.
    fn unread_targets(
        &self,
        specs: &[SpecResult],
        deferred: &mut Vec<Deferred>,
    ) -> Vec<Record<'f>> {
        let mut named = Vec::new();
        for (guid, nested) in std::mem::take(deferred) {
            if !self.is_docked(guid) {
                self.entry_records(&nested, &mut named, deferred);
            }
        }
        for spec in specs.iter().filter_map(|spec| spec.as_ref().as_ref().ok()) {
            named.extend(spec.default_targets().filter_map(|t| self.record_of(t)));
            if let Some(entries) = spec.manual_entries() {
                self.entry_records(entries, &mut named, deferred);
            }
        }
        let mut seen = HashSet::new();
        named.retain(|record| {
            !self.specs.contains_key(&record.index()) && seen.insert(record.index())
        });
        named
    }

    /// Collects the Records the entries name, nested ones included. When docked vehicles are not
    /// expanded, the entries nested below a target that is not read yet wait in `deferred` until
    /// the prefetch has read it, and those below a docked vehicle are left out.
    fn entry_records(
        &self,
        entries: &[EntrySpec],
        out: &mut Vec<Record<'f>>,
        deferred: &mut Vec<Deferred>,
    ) {
        let mut pending = vec![entries.iter()];
        while let Some(siblings) = pending.last_mut() {
            let Some(entry) = siblings.next() else {
                pending.pop();
                continue;
            };
            let record = self.record_of(&entry.target);
            out.extend(record);
            let Some(LoadoutSpec::Manual(nested)) = &entry.loadout else {
                continue;
            };
            let Some(record) = record.filter(|_| self.stop_at_docked) else {
                pending.push(nested.iter());
                continue;
            };
            match self.specs.get(&record.index()) {
                None => deferred.push((record.guid(), Arc::clone(nested.shared()))),
                Some(spec) if spec.as_ref().as_ref().is_ok_and(|s| s.docked_vehicle) => {}
                Some(_) => pending.push(nested.iter()),
            }
        }
    }

    fn is_docked(&self, guid: Guid) -> bool {
        self.entities
            .forge()
            .record_by_guid(guid)
            .and_then(|record| self.specs.get(&record.index()))
            .is_some_and(|spec| spec.as_ref().as_ref().is_ok_and(|s| s.docked_vehicle))
    }
}

#[cfg(test)]
#[path = "resolver_prefetch_stack_tests.rs"]
mod stack_tests;
