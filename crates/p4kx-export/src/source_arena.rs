// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DecimalU64, SourceUses};
use schemars::JsonSchema;
use serde::Serialize;

use crate::catalog_read::original::{
    OriginalFlowFields, OriginalRecordScope, OriginalValueEdge, OriginalValueEvent,
    OriginalValueSite,
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

impl<'source> SourceArena<'source> {
    fn event<'scope, 'read>(
        &mut self,
        event: &OriginalValueEvent<'_, 'scope, 'read, 'source>,
        sources: &SourceUses,
        scalar: &mut impl FnMut(&OriginalValueSite<'_, 'scope, 'read, 'source>),
    ) -> Result<(), CatalogError> {
        match event {
            OriginalValueEvent::Value(site) => self.visit(site, sources, scalar),
            OriginalValueEvent::EndContainer { ordinal, kind } => {
                let container = self
                    .open
                    .pop()
                    .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
                if container.ordinal != *ordinal {
                    return Err(ExportAccountingError::ConflictingSourceDatum.into());
                }
                let index = container.index;
                let value = container.finish(*kind, sources)?;
                let slot = self
                    .values
                    .get_mut(index)
                    .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
                if slot.ordinal != *ordinal || slot.value.is_some() {
                    return Err(ExportAccountingError::ConflictingSourceDatum.into());
                }
                slot.value = Some(value);
                Ok(())
            }
        }
    }

    fn visit<'scope, 'read>(
        &mut self,
        site: &OriginalValueSite<'_, 'scope, 'read, 'source>,
        sources: &SourceUses,
        scalar: &mut impl FnMut(&OriginalValueSite<'_, 'scope, 'read, 'source>),
    ) -> Result<(), CatalogError> {
        if site.ordinal() != self.next_ordinal
            || site.parent_ordinal() != self.open.last().map(|container| container.ordinal)
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
        self.attach(site, value, sources)?;
        if let Some(container) = SourceContainer::read(site, index, sources)? {
            self.values.push(ReservedValue {
                ordinal: site.ordinal(),
                value: None,
            });
            self.open.push(container);
        } else {
            scalar(site);
            self.values.push(ReservedValue {
                ordinal: site.ordinal(),
                value: Some(scalar_read::read(&site.fact(), sources)?),
            });
        }
        Ok(())
    }

    fn attach(
        &mut self,
        site: &OriginalValueSite<'_, '_, '_, 'source>,
        value: SourceValueRef,
        sources: &SourceUses,
    ) -> Result<(), CatalogError> {
        let field = match site.edge() {
            OriginalValueEdge::Field { .. } => {
                Some(field_read::read(&site.fact(), value, sources)?)
            }
            OriginalValueEdge::Element { .. } => None,
        };
        if let Some(parent) = self.open.last_mut() {
            return parent.push(site.edge(), value, field);
        }
        match (site.edge(), field) {
            (OriginalValueEdge::Field { ordinal }, Some(field))
                if u64::try_from(self.fields.len()).ok() == Some(ordinal) =>
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
