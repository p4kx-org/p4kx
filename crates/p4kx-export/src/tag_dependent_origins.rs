// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;
use std::collections::BTreeSet;
use std::fmt::{self, Debug, Formatter};
use std::sync::Arc;

use p4kx_extraction::{SourceEvidenceError, SourceUse, SourceUses, SourceValue, SourcedValue};
use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::{Serialize, Serializer};

use super::path_capture::{PathReceipts, PathUse};
use crate::CatalogError;

#[derive(Clone)]
struct SelectedPaths {
    receipts: Arc<PathReceipts>,
    first: usize,
    other: BTreeSet<usize>,
}

/// Occurrence-local uses plus positions in one actual selected Tag arena.
#[derive(Clone)]
pub(crate) struct TagInputs {
    local: Vec<SourceUse>,
    selected: Option<SelectedPaths>,
    complete: bool,
}

impl TagInputs {
    pub(crate) const fn local(local: Vec<SourceUse>, complete: bool) -> Self {
        Self {
            local,
            selected: None,
            complete,
        }
    }

    pub(super) fn selected(receipts: &Arc<PathReceipts>, position: usize, complete: bool) -> Self {
        Self {
            local: Vec::new(),
            selected: Some(SelectedPaths {
                receipts: Arc::clone(receipts),
                first: position,
                other: BTreeSet::new(),
            }),
            complete,
        }
    }

    pub(crate) const fn is_complete(&self) -> bool {
        self.complete
    }

    pub(crate) fn retain(&mut self, uses: impl IntoIterator<Item = SourceUse>, complete: bool) {
        self.local.extend(uses);
        self.complete &= complete;
    }

    pub(crate) fn merge(&mut self, other: Self) -> Result<(), CatalogError> {
        if let Some(selected) = other.selected {
            if let Some(current) = &mut self.selected {
                if !Arc::ptr_eq(&current.receipts, &selected.receipts) {
                    return Err(SourceEvidenceError::ForeignEvidence.into());
                }
                current.other.extend(
                    std::iter::once(selected.first)
                        .chain(selected.other)
                        .filter(|position| *position != current.first),
                );
            } else {
                self.selected = Some(selected);
            }
        }
        self.retain(other.local, other.complete);
        Ok(())
    }

    pub(crate) fn bind<T>(
        self,
        value: SourceValue<T>,
        sources: &SourceUses,
    ) -> Result<ContextualSourcedValue<T>, CatalogError> {
        let Some(selected) = &self.selected else {
            return Ok(ContextualSourcedValue::immediate(sources.value(
                value,
                &self.local,
                self.complete,
            )));
        };
        if !selected.receipts.uses.is_same_inventory(sources) {
            return Err(SourceEvidenceError::ForeignEvidence.into());
        }
        Ok(ContextualSourcedValue {
            value: Cell::Deferred {
                value,
                inputs: self,
                sources: sources.clone(),
            },
        })
    }
}

/// A value whose shared Tag lineage can be materialized by the writer.
pub(crate) struct ContextualSourcedValue<T> {
    value: Cell<T>,
}

enum Cell<T> {
    Immediate(SourcedValue<T>),
    Deferred {
        value: SourceValue<T>,
        inputs: TagInputs,
        sources: SourceUses,
    },
}

impl<T> ContextualSourcedValue<T> {
    pub(crate) const fn immediate(value: SourcedValue<T>) -> Self {
        Self {
            value: Cell::Immediate(value),
        }
    }
}

impl<T> Debug for ContextualSourcedValue<T> {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ContextualSourcedValue")
            .finish_non_exhaustive()
    }
}

impl<T: Clone + Serialize> Serialize for ContextualSourcedValue<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match &self.value {
            Cell::Immediate(value) => value.serialize(serializer),
            Cell::Deferred {
                value,
                inputs,
                sources,
            } => {
                let mut uses = inputs.local.clone();
                if let Some(selected) = &inputs.selected {
                    if selected.other.is_empty() {
                        uses.extend(
                            selected
                                .receipts
                                .lineage(selected.first, PathUse::Construction),
                        );
                    } else {
                        selected
                            .receipts
                            .append_path_union(
                                std::iter::once(selected.first)
                                    .chain(selected.other.iter().copied()),
                                &mut uses,
                            )
                            .map_err(serde::ser::Error::custom)?;
                    }
                }
                sources
                    .value(value.clone(), &uses, inputs.complete)
                    .serialize(serializer)
            }
        }
    }
}

impl<T: JsonSchema> JsonSchema for ContextualSourcedValue<T> {
    fn inline_schema() -> bool {
        SourcedValue::<T>::inline_schema()
    }

    fn schema_name() -> Cow<'static, str> {
        SourcedValue::<T>::schema_name()
    }

    fn schema_id() -> Cow<'static, str> {
        SourcedValue::<T>::schema_id()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        SourcedValue::<T>::json_schema(generator)
    }
}

#[cfg(test)]
#[path = "tag_dependent_origins_tests.rs"]
mod tests;
