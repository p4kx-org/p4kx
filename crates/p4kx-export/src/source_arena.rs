// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DecimalU64, SourceUses};
use schemars::JsonSchema;
use serde::Serialize;

use crate::catalog_read::original::{
    OriginalContainerKind, OriginalFlowFields, OriginalRecordScope, OriginalValueEdge,
    OriginalValueEvent, OriginalValueFact, OriginalValueSite,
};
use crate::schema_fields::every_field_required;
use crate::{CatalogError, ExportAccountingError};

use super::{SourceField, SourceNode, SourceValueRef};
use super::{field_read, scalar_read};

use super::container::SourceContainer;

struct ReservedValue<'source> {
    ordinal: u64,
    value: Option<SourceNode<'source>>,
}

/// One owner-local value inventory assembled only from the original walk's events.
#[derive(Default)]
struct SourceArena<'source> {
    fields: Vec<SourceField<'source>>,
    values: Vec<ReservedValue<'source>>,
    open: Vec<SourceContainer<'source>>,
    next_ordinal: u64,
    selected_root: Option<u64>,
}

/// The complete root fields and flat values of one physical original owner.
#[derive(Debug, Serialize, JsonSchema)]
#[schemars(transform = every_field_required)]
pub(crate) struct SourceArenaOutput<'source> {
    #[serde(rename = "roots")]
    pub(crate) fields: Vec<SourceField<'source>>,
    pub(crate) values: Vec<SourceNode<'source>>,
}

/// Retains one actual Record's complete ordered source inventory.
pub(crate) fn read<'scope, 'read, 'source>(
    scope: &'scope OriginalRecordScope<'read, 'source>,
    cancel: Option<&'scope CancelToken>,
    mut scalar: impl FnMut(&OriginalValueSite<'_, 'scope, 'read, 'source>),
) -> Result<SourceArenaOutput<'source>, CatalogError> {
    let mut arena = SourceArena::default();
    let mut walk = scope.walk(cancel);
    while let Some(event) = walk.next_event()? {
        arena.event(&event, scope.sources(), &mut scalar)?;
    }
    arena.finish()
}

/// One source arena and the actual facts that emitted it.
pub(crate) struct IndexedSourceArena<'scope, 'read, 'source> {
    pub(crate) facts: OriginalFlowFields<'scope, 'read, 'source>,
    pub(crate) source: SourceArenaOutput<'source>,
}

/// Retains the source arena and its typed fact index from one original walk.
pub(crate) fn read_indexed<'scope, 'read, 'source>(
    scope: &'scope OriginalRecordScope<'read, 'source>,
    cancel: Option<&'scope CancelToken>,
    mut scalar: impl FnMut(&OriginalValueSite<'_, 'scope, 'read, 'source>),
) -> Result<IndexedSourceArena<'scope, 'read, 'source>, CatalogError> {
    let mut arena = SourceArena::default();
    let facts = OriginalFlowFields::read_observed(scope, cancel, |event| {
        arena.event(event, scope.sources(), &mut scalar)
    })?;
    Ok(IndexedSourceArena {
        facts,
        source: arena.finish()?,
    })
}

/// Retains one original field's complete value subtree with arena-local links.
pub(crate) fn read_subtree<'scope, 'read, 'source>(
    fields: &OriginalFlowFields<'scope, 'read, 'source>,
    root: &OriginalValueFact<'scope, 'read, 'source>,
    mut retain: impl FnMut(&OriginalValueFact<'scope, 'read, 'source>) -> Result<(), CatalogError>,
) -> Result<SourceArenaOutput<'source>, CatalogError> {
    let span = fields.span(root)?;
    if !matches!(root.edge(), OriginalValueEdge::Field { .. }) {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    }
    let mut arena = SourceArena {
        next_ordinal: root.ordinal(),
        selected_root: Some(root.ordinal()),
        ..SourceArena::default()
    };
    let facts = fields
        .facts()
        .get(span)
        .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
    for fact in facts {
        fields.check_cancel()?;
        arena.close_to_parent(fact.parent_ordinal(), fields.sources())?;
        arena.visit(fact, fields.sources(), || retain(fact))?;
    }
    arena.close_to_parent(None, fields.sources())?;
    arena.finish()
}

impl<'source> SourceArena<'source> {
    fn event<'scope, 'read>(
        &mut self,
        event: &OriginalValueEvent<'_, 'scope, 'read, 'source>,
        sources: &SourceUses,
        scalar: &mut impl FnMut(&OriginalValueSite<'_, 'scope, 'read, 'source>),
    ) -> Result<(), CatalogError> {
        match event {
            OriginalValueEvent::Value(site) => self.visit(&site.fact(), sources, || {
                scalar(site);
                Ok(())
            }),
            OriginalValueEvent::EndContainer { ordinal, kind } => {
                self.close(*ordinal, *kind, sources)
            }
        }
    }

    fn close(
        &mut self,
        ordinal: u64,
        kind: OriginalContainerKind,
        sources: &SourceUses,
    ) -> Result<(), CatalogError> {
        let container = self
            .open
            .pop()
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        if container.ordinal != ordinal {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        let index = container.index;
        let value = container.finish(kind, sources)?;
        let slot = self
            .values
            .get_mut(index)
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        if slot.ordinal != ordinal || slot.value.is_some() {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        slot.value = Some(value);
        Ok(())
    }

    fn close_to_parent(
        &mut self,
        parent: Option<u64>,
        sources: &SourceUses,
    ) -> Result<(), CatalogError> {
        while let Some(container) = self.open.last() {
            if Some(container.ordinal) == parent {
                break;
            }
            self.close(container.ordinal, container.kind(), sources)?;
        }
        Ok(())
    }

    fn visit(
        &mut self,
        fact: &OriginalValueFact<'_, '_, 'source>,
        sources: &SourceUses,
        scalar: impl FnOnce() -> Result<(), CatalogError>,
    ) -> Result<(), CatalogError> {
        let root = self.selected_root == Some(fact.ordinal());
        if fact.ordinal() != self.next_ordinal
            || (!root
                && fact.parent_ordinal() != self.open.last().map(|container| container.ordinal))
        {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        self.next_ordinal = self
            .next_ordinal
            .checked_add(1)
            .ok_or(ExportAccountingError::CounterOverflow)?;
        let index = self.values.len();
        let value_index =
            u64::try_from(index).map_err(|_| ExportAccountingError::CounterOverflow)?;
        let value = SourceValueRef {
            value_index: DecimalU64::new(value_index),
        };
        self.attach(fact, value, sources)?;
        if let Some(container) = SourceContainer::read(fact, index, sources)? {
            self.values.push(ReservedValue {
                ordinal: fact.ordinal(),
                value: None,
            });
            self.open.push(container);
        } else {
            scalar()?;
            self.values.push(ReservedValue {
                ordinal: fact.ordinal(),
                value: Some(scalar_read::read(fact, sources)?),
            });
        }
        Ok(())
    }

    fn attach(
        &mut self,
        fact: &OriginalValueFact<'_, '_, 'source>,
        value: SourceValueRef,
        sources: &SourceUses,
    ) -> Result<(), CatalogError> {
        let field = match fact.edge() {
            OriginalValueEdge::Field { .. } => Some(field_read::read(fact, value, sources)?),
            OriginalValueEdge::Element { .. } => None,
        };
        if let Some(parent) = self.open.last_mut() {
            return parent.push(fact.edge(), value, field);
        }
        let ordinal = u64::try_from(self.fields.len()).ok();
        match (fact.edge(), field) {
            (OriginalValueEdge::Field { ordinal: original }, Some(field))
                if self.selected_root == Some(fact.ordinal()) || ordinal == Some(original) =>
            {
                self.fields.push(field);
            }
            _ => return Err(ExportAccountingError::ConflictingSourceDatum.into()),
        }
        Ok(())
    }

    fn finish(self) -> Result<SourceArenaOutput<'source>, CatalogError> {
        if let Some(container) = self.open.first() {
            return Err(ExportAccountingError::UnfilledLootValue {
                ordinal: container.ordinal,
            }
            .into());
        }
        let values = self
            .values
            .into_iter()
            .map(|reserved| {
                reserved.value.ok_or_else(|| {
                    CatalogError::from(ExportAccountingError::UnfilledLootValue {
                        ordinal: reserved.ordinal,
                    })
                })
            })
            .collect::<Result<_, _>>()?;
        Ok(SourceArenaOutput {
            fields: self.fields,
            values,
        })
    }
}
