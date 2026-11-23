// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, HashSet};

use p4kx_extraction::{CancelToken, DataForge, Guid};

use crate::catalog_query_session::checkpoint;
use crate::{CatalogError, ExportAccountingError};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ClassTarget {
    One(u32),
    Ambiguous,
}

pub(crate) struct ClassMembership {
    pub(crate) configured: bool,
    pub(crate) complete: bool,
}

pub(crate) type ClassLookup<'lookup> =
    dyn FnMut(u32) -> Result<ClassMembership, CatalogError> + 'lookup;

enum Configured<'lookup> {
    Indexed(HashSet<u32>),
    Scoped(&'lookup mut ClassLookup<'lookup>),
}

#[derive(Debug, PartialEq, Eq)]
struct ClassSurvey {
    target: Option<ClassTarget>,
    complete: bool,
}

pub(super) struct ClassTargets<'source, 'lookup> {
    forge: &'source DataForge<'source>,
    configured: Configured<'lookup>,
    resolved: HashMap<Guid, Option<ClassTarget>>,
    cancel: Option<&'source CancelToken>,
}

impl<'source, 'lookup> ClassTargets<'source, 'lookup> {
    pub(super) fn new(
        forge: &'source DataForge<'source>,
        configured: impl IntoIterator<Item = u32>,
        cancel: Option<&'source CancelToken>,
    ) -> Result<Self, CatalogError> {
        let mut physical = HashSet::new();
        for index in configured {
            checkpoint(cancel)?;
            if forge.record(index).is_none() {
                return Err(ExportAccountingError::ConflictingSourceDatum.into());
            }
            physical
                .try_reserve(1)
                .map_err(CatalogError::SourceUseAllocation)?;
            physical.insert(index);
        }
        Ok(Self {
            forge,
            configured: Configured::Indexed(physical),
            resolved: HashMap::new(),
            cancel,
        })
    }

    pub(super) fn scoped(
        forge: &'source DataForge<'source>,
        cancel: Option<&'source CancelToken>,
        lookup: &'lookup mut ClassLookup<'lookup>,
    ) -> Self {
        Self {
            forge,
            configured: Configured::Scoped(lookup),
            resolved: HashMap::new(),
            cancel,
        }
    }

    pub(super) fn resolve(&mut self, guid: Guid) -> Result<Option<ClassTarget>, CatalogError> {
        checkpoint(self.cancel)?;
        if let Some(target) = self.resolved.get(&guid) {
            return Ok(*target);
        }
        let result = survey(
            self.forge
                .records_by_guid(guid)
                .map(|record| record.index()),
            |index| {
                checkpoint(self.cancel)?;
                match &mut self.configured {
                    Configured::Indexed(configured) => Ok(ClassMembership {
                        configured: configured.contains(&index),
                        complete: true,
                    }),
                    Configured::Scoped(lookup) => lookup(index),
                }
            },
        )?;
        if !result.complete && matches!(self.configured, Configured::Indexed(_)) {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        self.resolved
            .try_reserve(1)
            .map_err(CatalogError::SourceUseAllocation)?;
        self.resolved.insert(guid, result.target);
        Ok(result.target)
    }
}

fn survey(
    records: impl IntoIterator<Item = u32>,
    mut membership: impl FnMut(u32) -> Result<ClassMembership, CatalogError>,
) -> Result<ClassSurvey, CatalogError> {
    let mut candidates = records.into_iter();
    let mut target = None;
    let mut complete = true;
    if let Some(first) = candidates.next() {
        let initial = membership(first)?;
        let mut configured = initial.configured;
        let mut aliased = false;
        complete = initial.complete;
        for index in candidates {
            let candidate = membership(index)?;
            configured |= candidate.configured;
            complete &= candidate.complete;
            aliased = true;
        }
        if configured {
            target = Some(if aliased {
                ClassTarget::Ambiguous
            } else {
                ClassTarget::One(first)
            });
        }
    }
    Ok(ClassSurvey { target, complete })
}

#[cfg(test)]
#[path = "facility_interior_class_targets_tests.rs"]
mod tests;
