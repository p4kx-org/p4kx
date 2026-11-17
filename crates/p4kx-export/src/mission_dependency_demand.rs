// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

use p4kx_extraction::Guid;

use crate::mission_index::read::selection::Action;
use crate::record_read::ReadError;
use crate::{CatalogError, ExportAccountingError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum DependencyKey {
    Mission(Action, Guid),
    MissionItem { guid: Guid, record_index: u32 },
    Place(Guid),
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum PreparedOutcome {
    Proven,
    Absent,
    Failed,
}

#[derive(Debug, Clone, Copy, thiserror::Error)]
#[error("a canonical mission dependency requires preparation")]
pub(crate) struct DependencyRequired {
    pub(crate) key: DependencyKey,
}

/// Complete producer inputs or shared progress for one selective query.
#[derive(Debug, Clone, Default)]
pub(crate) struct PreparedDependencies(Option<Arc<Mutex<Progress>>>);

#[derive(Debug, Default)]
struct Progress {
    prepared: BTreeMap<DependencyKey, PreparedOutcome>,
    consulted: BTreeSet<DependencyKey>,
}

impl PreparedDependencies {
    pub(crate) fn query() -> Self {
        Self(Some(Arc::new(Mutex::new(Progress::default()))))
    }

    pub(crate) fn require(&self, key: DependencyKey) -> Result<(), ReadError> {
        let Some(prepared) = &self.0 else {
            return Ok(());
        };
        let mut prepared = prepared
            .lock()
            .map_err(|_| CatalogError::from(ExportAccountingError::ConflictingSourceDatum))?;
        prepared.consulted.insert(key);
        if prepared.prepared.contains_key(&key) {
            Ok(())
        } else {
            Err(Box::new(DependencyRequired { key }))
        }
    }

    pub(crate) fn begin_attempt(&self) -> Result<(), CatalogError> {
        let prepared = self
            .0
            .as_ref()
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        prepared
            .lock()
            .map_err(|_| ExportAccountingError::ConflictingSourceDatum)?
            .consulted
            .clear();
        Ok(())
    }

    pub(crate) fn consulted(&self) -> Result<BTreeSet<DependencyKey>, CatalogError> {
        let prepared = self
            .0
            .as_ref()
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        Ok(prepared
            .lock()
            .map_err(|_| ExportAccountingError::ConflictingSourceDatum)?
            .consulted
            .clone())
    }

    pub(crate) fn prepare(
        &self,
        key: DependencyKey,
        outcome: PreparedOutcome,
    ) -> Result<(), CatalogError> {
        let prepared = self
            .0
            .as_ref()
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        let mut prepared = prepared
            .lock()
            .map_err(|_| ExportAccountingError::ConflictingSourceDatum)?;
        match prepared.prepared.entry(key) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(outcome);
                Ok(())
            }
            std::collections::btree_map::Entry::Occupied(_) => {
                Err(ExportAccountingError::ConflictingSourceDatum.into())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{DependencyKey, DependencyRequired, PreparedDependencies, PreparedOutcome};
    use crate::mission_index::read::selection::Action;
    use crate::record_read::ReadError;
    use p4kx_extraction::Guid;

    #[test]
    fn demand_is_typed_and_only_an_actual_preparation_allows_absence() -> Result<(), ReadError> {
        let key = DependencyKey::Mission(Action::Template, Guid::default());
        PreparedDependencies::default().require(key)?;
        let prepared = PreparedDependencies::query();
        let request = prepared
            .require(key)
            .expect_err("an unknown input cannot become None");
        assert!(
            matches!(request.downcast_ref::<DependencyRequired>(), Some(required) if required.key == key),
            "the original typed key survives the private error path"
        );
        prepared.prepare(key, PreparedOutcome::Absent)?;
        prepared.require(key)?;
        assert!(
            prepared.prepare(key, PreparedOutcome::Proven).is_err(),
            "a repeated prepared demand is an invariant failure rather than a retry loop"
        );
        let other = DependencyKey::Mission(Action::Broker, Guid::default());
        assert!(
            prepared.require(other).is_err(),
            "families with the same nominal identity remain independent"
        );
        let item = DependencyKey::MissionItem {
            guid: Guid::default(),
            record_index: 0,
        };
        assert!(
            prepared.require(item).is_err(),
            "a physical MissionItem is its own dependency key"
        );
        prepared.prepare(item, PreparedOutcome::Failed)?;
        prepared.require(item)?;
        Ok(())
    }
}
