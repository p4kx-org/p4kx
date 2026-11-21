// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::collections::hash_map::Entry as Slot;

use p4kx_dataforge::{DataForge, DataForgeError, Record, TreeBuilder};
use rayon::prelude::*;

use super::{
    TriggerOccurrence, TriggerReference, TriggerScope, positioned_trigger_references_in,
    trigger_references,
};
use crate::entities::is_entity;

/// Which Entities name each trigger, in definition order.
#[derive(Debug, Default)]
pub struct TriggerUsers<Evidence = ()> {
    matches: HashMap<Box<str>, Vec<Member<Evidence>>>,
    inspected: Vec<u32>,
    failed: Vec<u32>,
    incomplete: bool,
    entities: usize,
}

#[derive(Debug)]
struct Member<Evidence> {
    record: u32,
    evidence: Evidence,
}

struct Read<Evidence> {
    references: Vec<(TriggerReference, Evidence)>,
    incomplete: bool,
}

impl TriggerUsers {
    /// Reads each Entity once and retains the Records that name a wanted lower-case trigger.
    #[must_use]
    pub fn build(forge: &DataForge<'_>, wanted: &(dyn Fn(&str) -> bool + Sync)) -> Self {
        Self::scan(forge, wanted, |builder, record| {
            let tree = builder.build(record)?;
            Ok(Read {
                references: trigger_references(&tree)
                    .into_iter()
                    .map(|reference| (reference, ()))
                    .collect(),
                incomplete: tree.errors().next().is_some(),
            })
        })
    }

    /// Reads each Entity once and retains the first authored field proving each match.
    #[must_use]
    pub fn build_positioned(
        forge: &DataForge<'_>,
        wanted: &(dyn Fn(&str) -> bool + Sync),
    ) -> TriggerUsers<Option<TriggerOccurrence>> {
        TriggerUsers::scan(forge, wanted, positioned)
    }
}

impl<E: Send + Default> TriggerUsers<E> {
    fn scan<'f>(
        forge: &'f DataForge<'f>,
        wanted: &(dyn Fn(&str) -> bool + Sync),
        read: impl Fn(&mut TreeBuilder<'f>, &Record<'f>) -> Result<Read<E>, DataForgeError> + Sync,
    ) -> Self {
        let records: Vec<Record<'_>> = forge.records().filter(is_entity).collect();
        let mut result = records
            .par_iter()
            .fold(
                || (TreeBuilder::new(), Self::default()),
                |(mut builder, mut found), record| {
                    match read(&mut builder, record) {
                        Ok(read) => found.admit(record.index(), read, wanted),
                        Err(_) => found.failed.push(record.index()),
                    }
                    (builder, found)
                },
            )
            .map(|(_, partial)| partial)
            .reduce(Self::default, Self::merge);
        for users in result.matches.values_mut() {
            users.sort_unstable_by_key(|user| user.record);
            users.dedup_by_key(|user| user.record);
            users.shrink_to_fit();
        }
        result.inspected.sort_unstable();
        result.failed.sort_unstable();
        result.entities = records.len();
        result
    }

    fn admit(&mut self, record: u32, read: Read<E>, wanted: &dyn Fn(&str) -> bool) {
        self.inspected.push(record);
        self.incomplete |= read.incomplete;
        for (reference, evidence) in read.references {
            let key = reference.name().to_ascii_lowercase();
            if wanted(&key) {
                self.matches
                    .entry(key.into_boxed_str())
                    .or_default()
                    .push(Member { record, evidence });
            }
        }
    }

    fn merge(mut self, other: Self) -> Self {
        for (key, mut users) in other.matches {
            match self.matches.entry(key) {
                Slot::Occupied(slot) => slot.into_mut().append(&mut users),
                Slot::Vacant(slot) => {
                    slot.insert(users);
                }
            }
        }
        self.inspected.extend(other.inspected);
        self.failed.extend(other.failed);
        self.incomplete |= other.incomplete;
        self
    }
}

impl<E> TriggerUsers<E> {
    /// Returns how many Entity Records were inspected or attempted.
    #[must_use]
    pub const fn entity_count(&self) -> usize {
        self.entities
    }

    /// Returns how many Entity Records could not be read.
    #[must_use]
    pub const fn unreadable_records(&self) -> usize {
        self.failed.len()
    }

    /// Returns the original canonical Record ordinals successfully inspected.
    #[must_use]
    pub fn inspected_records(&self) -> &[u32] {
        &self.inspected
    }

    /// Returns the original canonical Record ordinals whose reads failed.
    #[must_use]
    pub fn failed_records(&self) -> &[u32] {
        &self.failed
    }

    /// Returns whether all admitted Entity Records could be fully inspected.
    #[must_use]
    pub const fn complete(&self) -> bool {
        !self.incomplete && self.failed.is_empty()
    }

    /// Returns how many Entities name the trigger, ignoring ASCII case.
    #[must_use]
    pub fn count(&self, trigger: &str) -> usize {
        self.members(trigger).len()
    }

    /// Returns the Entities naming the trigger, in definition order.
    pub fn entities<'f>(
        &self,
        forge: &'f DataForge<'f>,
        trigger: &str,
    ) -> impl Iterator<Item = Record<'f>> + use<'_, 'f, E> {
        self.observed_entities(forge, trigger)
            .map(|(record, _)| record)
    }

    /// Returns each matching Entity with the evidence retained by the selected scan.
    pub fn observed_entities<'f>(
        &self,
        forge: &'f DataForge<'f>,
        trigger: &str,
    ) -> impl Iterator<Item = (Record<'f>, &E)> + use<'_, 'f, E> {
        self.members(trigger).iter().filter_map(|user| {
            forge
                .record(user.record)
                .map(|record| (record, &user.evidence))
        })
    }

    fn members(&self, trigger: &str) -> &[Member<E>] {
        self.matches
            .get(trigger.to_ascii_lowercase().as_str())
            .map_or(&[], Vec::as_slice)
    }
}

fn positioned<'f>(
    builder: &mut TreeBuilder<'f>,
    record: &Record<'f>,
) -> Result<Read<Option<TriggerOccurrence>>, DataForgeError> {
    let tree = builder.build_positioned_partial(record)?;
    let references = positioned_trigger_references_in(&tree, TriggerScope::default());
    let incomplete = tree.tree().errors().next().is_some() || references.is_err();
    let references = references.map_or_else(
        |_| {
            trigger_references(tree.tree())
                .into_iter()
                .map(|reference| (reference, None))
                .collect()
        },
        |references| {
            references
                .into_iter()
                .map(|reference| {
                    let (reference, sites) = reference.into_parts();
                    (reference, sites.into_iter().next())
                })
                .collect()
        },
    );
    Ok(Read {
        references,
        incomplete,
    })
}
