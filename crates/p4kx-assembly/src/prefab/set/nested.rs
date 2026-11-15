// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::sync::Arc;

use p4kx_model::{PrefabLibrary, PrefabRef};

use super::{Failure, GuidIndex, PrefabFile, PrefabSet};
use crate::prefab::resolution_source::OwnedPrefabLocation;
use crate::prefab::{PrefabError, PrefabLinkSource, PrefabLookupRoute};

const GUID_INDEX_THREADS: usize = 4;

pub(in crate::prefab) enum Lookup {
    Found(Arc<PrefabFile>, usize),
    Missing(Option<(String, String)>),
}

pub(in crate::prefab) struct NestedLookup {
    pub(in crate::prefab) outcome: Lookup,
    pub(in crate::prefab) source: Option<Arc<PrefabLinkSource>>,
}

#[derive(Default)]
struct LookupObservations {
    surveyed_names: Vec<Arc<PrefabFile>>,
    failed_entry_indices: Vec<usize>,
}

impl PrefabSet<'_> {
    fn by_name(
        &mut self,
        reference: &PrefabRef,
        observations: &mut LookupObservations,
    ) -> Result<Lookup, PrefabError> {
        let mut unreadable = None;
        for (at, _) in reference.name.match_indices('.') {
            let Some((library, rest)) = reference.name.split_at_checked(at) else {
                continue;
            };
            let prefab = rest.trim_start_matches('.');
            let Some(slot) = self.slot_of(library) else {
                continue;
            };
            if prefab.is_empty() {
                continue;
            }
            match self.load(slot) {
                Ok(file) => {
                    if let Some(index) = file.find(prefab) {
                        return Ok(Lookup::Found(file, index));
                    }
                    if self.captures_objects() {
                        observations.surveyed_names.push(file);
                    }
                }
                Err(PrefabError::Unreadable { library, reason }) => {
                    self.observe_failed(slot, observations);
                    unreadable.get_or_insert((library, reason));
                }
                Err(error) => return Err(error),
            }
        }
        Ok(Lookup::Missing(unreadable))
    }

    fn guid_index(&mut self) -> Result<&GuidIndex, PrefabError> {
        use rayon::iter::{IntoParallelIterator as _, ParallelIterator as _};

        if self.guids.is_none() {
            let ids_of = |slot: usize| match self.loaded.get(&slot) {
                Some(file) => Ok(ids(file.library())),
                None if self.failed.contains_key(&slot) => Ok(Vec::new()),
                None => self
                    .read(slot, PrefabLibrary::parse)
                    .map(|library| ids(&library)),
            };
            let count = self.slots.len();
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(GUID_INDEX_THREADS)
                .build();
            let read: Vec<Result<Vec<String>, Failure>> = pool.map_or_else(
                |_| (0..count).map(ids_of).collect(),
                |pool| pool.install(|| (0..count).into_par_iter().map(ids_of).collect()),
            );
            let mut index: GuidIndex = HashMap::new();
            for (slot, result) in read.into_iter().enumerate() {
                match result {
                    Ok(found) => {
                        for (prefab, id) in found.into_iter().enumerate() {
                            if !id.is_empty() {
                                index.entry(id).or_default().push((slot, prefab));
                            }
                        }
                    }
                    Err(Failure::Fatal(error)) => return Err(PrefabError::Archive(error)),
                    Err(Failure::Unreadable(reason)) => {
                        self.failed.insert(slot, reason);
                    }
                }
            }
            self.guids = Some(index);
        }
        Ok(self.guids.get_or_insert_with(HashMap::new))
    }

    /// Finds the prefab an object instances: the one of its `PrefabName` when that has the
    /// object's `PrefabGUID` or the object names no GUID, else the one with the GUID, else the
    /// one of the name.
    pub(in crate::prefab) fn find_nested(
        &mut self,
        reference: &PrefabRef,
    ) -> Result<NestedLookup, PrefabError> {
        let mut observations = LookupObservations::default();
        let named = self.by_name(reference, &mut observations)?;
        let guid = reference.guid.to_ascii_lowercase();
        if guid.is_empty() {
            let route = match named {
                Lookup::Found(..) => PrefabLookupRoute::Name,
                Lookup::Missing(_) => PrefabLookupRoute::UnmatchedName,
            };
            return Ok(self.finish_lookup(named, route, None, observations));
        }
        if let Lookup::Found(file, index) = &named
            && file
                .library()
                .prefabs
                .get(*index)
                .is_some_and(|prefab| prefab.id.eq_ignore_ascii_case(&guid))
        {
            return Ok(self.finish_lookup(
                named,
                PrefabLookupRoute::NameAndGuid,
                None,
                observations,
            ));
        }
        let candidates = self.guid_index()?.get(&guid).cloned().unwrap_or_default();
        let preferred = match &named {
            Lookup::Found(file, _) => candidates
                .iter()
                .copied()
                .find(|(slot, _)| *slot == file.slot()),
            Lookup::Missing(_) => None,
        };
        let others = candidates
            .into_iter()
            .filter(|candidate| Some(*candidate) != preferred);
        for (slot, index) in preferred.into_iter().chain(others) {
            if let Ok(file) = self.load(slot) {
                let compared_name = if self.captures_objects() {
                    located(&named)
                } else {
                    None
                };
                return Ok(self.finish_lookup(
                    Lookup::Found(file, index),
                    PrefabLookupRoute::GuidIndex,
                    compared_name,
                    observations,
                ));
            }
            self.observe_failed(slot, &mut observations);
        }
        let route = match named {
            Lookup::Found(..) => PrefabLookupRoute::NameFallback,
            Lookup::Missing(_) => PrefabLookupRoute::UnmatchedGuid,
        };
        Ok(self.finish_lookup(named, route, None, observations))
    }

    fn observe_failed(&self, slot: usize, observations: &mut LookupObservations) {
        if self.captures_objects()
            && let Some(found) = self.slots.get(slot)
        {
            observations.failed_entry_indices.push(found.entry.index());
        }
    }

    fn finish_lookup(
        &self,
        outcome: Lookup,
        route: PrefabLookupRoute,
        compared_name: Option<OwnedPrefabLocation>,
        observations: LookupObservations,
    ) -> NestedLookup {
        let source = self.captures_objects().then(|| {
            Arc::new(PrefabLinkSource {
                route,
                target: located(&outcome),
                compared_name,
                surveyed_names: observations.surveyed_names,
                failed_entry_indices: observations.failed_entry_indices,
            })
        });
        NestedLookup { outcome, source }
    }
}

fn located(lookup: &Lookup) -> Option<OwnedPrefabLocation> {
    let Lookup::Found(file, index) = lookup else {
        return None;
    };
    Some(OwnedPrefabLocation {
        file: Arc::clone(file),
        prefab_index: *index,
    })
}

fn ids(library: &PrefabLibrary) -> Vec<String> {
    library
        .prefabs
        .iter()
        .map(|prefab| prefab.id.to_ascii_lowercase())
        .collect()
}
