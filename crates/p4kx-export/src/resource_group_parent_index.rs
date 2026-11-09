// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use p4kx_extraction::{CancelToken, Guid, SourceUse};

use crate::CatalogError;

use super::parent_source::{ParentSource, ParentTarget};
use crate::export_ledger::ExportAccountingError;

#[derive(Debug)]
struct Occurrence {
    source: usize,
    ordinal: usize,
}

/// The existing first-selected non-self parent map with its original construction inputs.
#[derive(Debug, Default)]
pub(super) struct Parents {
    parents: HashMap<Guid, Occurrence>,
    sources: Vec<ParentSource>,
}

pub(crate) struct ParentChoice<'a> {
    pub(crate) target: Option<&'a ParentTarget>,
    pub(crate) inputs: Vec<SourceUse>,
    pub(crate) complete: bool,
}

impl Parents {
    pub(super) fn add(
        &mut self,
        source: ParentSource,
        children: Vec<(usize, Guid)>,
        cancel: Option<&CancelToken>,
    ) -> Result<(), CatalogError> {
        let position = self.sources.len();
        if let Some(target) = &source.target {
            for (ordinal, child) in children {
                if cancel.is_some_and(CancelToken::is_cancelled) {
                    return Err(CatalogError::Cancelled);
                }
                if child != target.guid.0 {
                    self.parents.entry(child).or_insert(Occurrence {
                        source: position,
                        ordinal,
                    });
                }
            }
        }
        self.sources.push(source);
        Ok(())
    }

    pub(super) fn failed(&mut self, source: ParentSource) {
        self.sources.push(source);
    }

    pub(super) fn choice(
        &self,
        child: &Guid,
        cancel: Option<&CancelToken>,
    ) -> Result<ParentChoice<'_>, CatalogError> {
        let chosen = self.parents.get(child);
        let target = chosen
            .map(|chosen| {
                self.sources
                    .get(chosen.source)
                    .and_then(|source| source.target.as_ref())
                    .ok_or(ExportAccountingError::ConflictingFieldInspection)
            })
            .transpose()?;
        let mut inputs = Vec::new();
        let mut complete = true;
        for (position, source) in self.sources.iter().enumerate() {
            if cancel.is_some_and(CancelToken::is_cancelled) {
                return Err(CatalogError::Cancelled);
            }
            let through = chosen
                .filter(|chosen| chosen.source == position)
                .map(|chosen| chosen.ordinal);
            complete &= source.append(*child, through, &mut inputs, cancel)?;
            if through.is_some() {
                break;
            }
        }
        Ok(ParentChoice {
            target,
            inputs,
            complete,
        })
    }
}
