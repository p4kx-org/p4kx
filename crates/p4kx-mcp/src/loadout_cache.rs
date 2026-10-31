// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::{Arc, Mutex, PoisonError};

use p4kx_assembly::{EntityNode, LoadoutPicks};

use crate::error::ToolError;

/// How many ports the trees kept for one Archive hold together.
const PORT_BUDGET: usize = 64 * 1024;
/// How many trees are kept for one Archive, however small.
const MAX_TREES: usize = 8;

/// What decides a resolved tree: the Entity as the call wrote it, whether docked vehicles are
/// left unresolved and the Loadout picks. The depth only cuts the tree when it is listed.
#[derive(Debug)]
struct Key {
    entity: String,
    stop_at_docked: bool,
    picks: LoadoutPicks,
}

/// The choices of a call besides the Entity that change the resolved tree.
type Variant<'a> = (bool, &'a LoadoutPicks);

type Kept<T> = Vec<(Key, usize, Arc<T>)>;

/// The Loadout trees resolved from one Archive, most recently used first, within a budget of
/// ports, so that paging through a tree resolves it once. A tree with more ports than the budget
/// is not kept.
#[derive(Debug)]
pub(crate) struct Cache<T> {
    kept: Mutex<Kept<T>>,
    budget: usize,
    weigh: fn(&T) -> usize,
}

/// The resolved Loadout trees of one Archive.
pub(crate) type LoadoutCache = Cache<EntityNode>;

impl Default for LoadoutCache {
    fn default() -> Self {
        Self::new(PORT_BUDGET, |tree| tree.walk().len())
    }
}

impl<T> Cache<T> {
    fn new(budget: usize, weigh: fn(&T) -> usize) -> Self {
        Self {
            kept: Mutex::default(),
            budget,
            weigh,
        }
    }

    /// Returns the tree of `entity`, resolving it with `resolve` when it is not kept.
    ///
    /// `resolve` runs without the lock; calls that miss at the same time each resolve, and the
    /// first tree kept wins.
    pub(crate) fn get_or_resolve(
        &self,
        entity: &str,
        variant: Variant<'_>,
        resolve: impl FnOnce() -> Result<T, ToolError>,
    ) -> Result<Arc<T>, ToolError> {
        if let Some(tree) = self.touch(entity, variant) {
            return Ok(tree);
        }
        let tree = Arc::new(resolve()?);
        let weight = (self.weigh)(&tree);
        let mut kept = self.kept.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(index) = position(&kept, entity, variant) {
            let known = kept.remove(index);
            let tree = Arc::clone(&known.2);
            kept.insert(0, known);
            return Ok(tree);
        }
        if weight <= self.budget {
            let key = Key {
                entity: entity.to_owned(),
                stop_at_docked: variant.0,
                picks: variant.1.clone(),
            };
            kept.insert(0, (key, weight, Arc::clone(&tree)));
            kept.truncate(MAX_TREES);
            let mut total = 0_usize;
            let fits = kept
                .iter()
                .take_while(|(_, weight, _)| {
                    total = total.saturating_add(*weight);
                    total <= self.budget
                })
                .count();
            kept.truncate(fits);
        }
        drop(kept);
        Ok(tree)
    }

    fn touch(&self, entity: &str, variant: Variant<'_>) -> Option<Arc<T>> {
        let mut kept = self.kept.lock().unwrap_or_else(PoisonError::into_inner);
        let index = position(&kept, entity, variant)?;
        let known = kept.remove(index);
        let tree = Arc::clone(&known.2);
        kept.insert(0, known);
        drop(kept);
        Some(tree)
    }
}

fn position<T>(
    kept: &Kept<T>,
    entity: &str,
    (stop_at_docked, picks): Variant<'_>,
) -> Option<usize> {
    kept.iter().position(|(key, _, _)| {
        key.entity == entity && key.stop_at_docked == stop_at_docked && key.picks == *picks
    })
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::sync::Arc;

    use p4kx_assembly::LoadoutPicks;

    use super::Cache;
    use crate::error::ToolError;

    const NONE: LoadoutPicks = LoadoutPicks::new();

    #[expect(
        clippy::unnecessary_wraps,
        reason = "the shape of the closure the cache takes"
    )]
    fn resolved(calls: &Cell<usize>, ports: usize) -> Result<Vec<u8>, ToolError> {
        calls.set(calls.get() + 1);
        Ok(vec![0; ports])
    }

    #[test]
    fn a_kept_tree_is_resolved_once_and_returned_again() -> Result<(), ToolError> {
        let cache = Cache::<Vec<u8>>::new(100, Vec::len);
        let calls = Cell::new(0);

        let first = cache.get_or_resolve("Titan", (false, &NONE), || resolved(&calls, 10))?;
        let second = cache.get_or_resolve("Titan", (false, &NONE), || resolved(&calls, 10))?;
        let docked = cache.get_or_resolve("Titan", (true, &NONE), || resolved(&calls, 10))?;

        assert_eq!(calls.get(), 2, "once per Entity and docked setting");
        assert!(Arc::ptr_eq(&first, &second));
        assert!(!Arc::ptr_eq(&first, &docked));
        Ok(())
    }

    #[test]
    fn the_picks_are_part_of_the_key() -> Result<(), ToolError> {
        let cache = Cache::<Vec<u8>>::new(100, Vec::len);
        let calls = Cell::new(0);
        let second = LoadoutPicks::new().group("Body", 1);

        cache.get_or_resolve("Beast", (false, &NONE), || resolved(&calls, 10))?;
        cache.get_or_resolve("Beast", (false, &second), || resolved(&calls, 10))?;
        cache.get_or_resolve("Beast", (false, &second), || resolved(&calls, 10))?;

        assert_eq!(calls.get(), 2, "once per set of picks");
        Ok(())
    }

    #[test]
    fn the_port_budget_drops_the_least_recently_used_tree() -> Result<(), ToolError> {
        let cache = Cache::<Vec<u8>>::new(100, Vec::len);
        let calls = Cell::new(0);

        cache.get_or_resolve("a", (false, &NONE), || resolved(&calls, 60))?;
        cache.get_or_resolve("b", (false, &NONE), || resolved(&calls, 60))?;
        cache.get_or_resolve("b", (false, &NONE), || resolved(&calls, 60))?;
        cache.get_or_resolve("a", (false, &NONE), || resolved(&calls, 60))?;

        assert_eq!(calls.get(), 3, "a was dropped for b and resolved again");
        Ok(())
    }

    #[test]
    fn a_tree_over_the_budget_is_not_kept() -> Result<(), ToolError> {
        let cache = Cache::<Vec<u8>>::new(100, Vec::len);
        let calls = Cell::new(0);

        cache.get_or_resolve("huge", (false, &NONE), || resolved(&calls, 101))?;
        cache.get_or_resolve("huge", (false, &NONE), || resolved(&calls, 101))?;

        assert_eq!(calls.get(), 2);
        Ok(())
    }
}
