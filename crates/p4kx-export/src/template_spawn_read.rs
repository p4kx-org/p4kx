// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;
use std::collections::HashMap;

use p4kx_extraction::{RecordTree, Value};

use crate::catalog_read::original::{OriginalFlowFields, OriginalValueFact};
use crate::record_read::ReadError;
use crate::source_fields::FieldCapture;
use crate::spawn_pools::{Registered, SpawnPools};
use crate::spawn_read::Reader;
use crate::template_reference_index::TemplateReferences;
use crate::template_source_members::TemplateMembers;
use crate::template_spawn_context::TemplateSpawnContext;
use crate::template_spawn_projection::TemplateSpawnProjection;
use crate::template_tag_cache::TemplateTagCache;
use crate::{CatalogError, ExportAccountingError};

#[path = "template_spawn_pending.rs"]
mod pending;

use pending::PendingSpawn;

/// Projected spawn bindings, their shared pools, and exact original failures.
pub(crate) struct SpawnEmission<'source> {
    pub(crate) projections: Vec<TemplateSpawnProjection>,
    pub(crate) pools: SpawnPools<'source>,
    pub(crate) failures: Vec<(u64, ReadError)>,
}

pub(crate) struct TemplateSpawns<'fields, 'scope, 'read, 'source> {
    pending: Vec<PendingSpawn<'fields, 'scope, 'read, 'source>>,
    fields: &'fields OriginalFlowFields<'scope, 'read, 'source>,
    registered: HashMap<u64, Registered>,
    pub(crate) pools: SpawnPools<'source>,
}

impl<'fields, 'scope, 'read, 'source> TemplateSpawns<'fields, 'scope, 'read, 'source> {
    pub(crate) fn new(fields: &'fields OriginalFlowFields<'scope, 'read, 'source>) -> Self {
        Self {
            fields,
            registered: HashMap::new(),
            pending: Vec::new(),
            pools: SpawnPools::default(),
        }
    }

    pub(crate) fn read(
        fields: &'fields OriginalFlowFields<'scope, 'read, 'source>,
        references: &RefCell<TemplateReferences<'fields, 'scope, 'read, 'source>>,
        tags: &TemplateTagCache<'_, 'fields, 'scope, 'read, 'source>,
        tree: &RecordTree<'source>,
        values: &[&'fields OriginalValueFact<'scope, 'read, 'source>],
        capture: &dyn FieldCapture<'source>,
    ) -> Result<Self, CatalogError> {
        let mut result = Self::new(fields);
        for fact in values {
            fields.check_cancel()?;
            if !PendingSpawn::is_spawn(fact.value()) {
                continue;
            }
            let pending = result.one(references, tags, tree, fact, capture)?;
            result.pending.push(pending);
        }
        Ok(result)
    }

    pub(crate) fn required(
        &mut self,
        references: &RefCell<TemplateReferences<'fields, 'scope, 'read, 'source>>,
        tags: &TemplateTagCache<'_, 'fields, 'scope, 'read, 'source>,
        tree: &RecordTree<'source>,
        fact: &'fields OriginalValueFact<'scope, 'read, 'source>,
        capture: &dyn FieldCapture<'source>,
    ) -> Result<(), ReadError> {
        self.fields.check_cancel()?;
        if self.registration(fact)?.is_some() {
            return Ok(());
        }
        let pending = self
            .one(references, tags, tree, fact, capture)?
            .required()?;
        self.pending.push(pending);
        Ok(())
    }

    fn one(
        &mut self,
        references: &RefCell<TemplateReferences<'fields, 'scope, 'read, 'source>>,
        tags: &TemplateTagCache<'_, 'fields, 'scope, 'read, 'source>,
        tree: &RecordTree<'source>,
        fact: &'fields OriginalValueFact<'scope, 'read, 'source>,
        capture: &dyn FieldCapture<'source>,
    ) -> Result<PendingSpawn<'fields, 'scope, 'read, 'source>, CatalogError> {
        let fields = self.fields;
        let Value::Struct(source) = fact.value() else {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        };
        let context = TemplateSpawnContext::new(fields, references, tags, Some(fact), capture)?;
        let parsed = Reader::for_context(tags.tree(), tree, &context)
            .and_then(|reader| reader.spawn(source));
        let parsed = match parsed {
            Ok(Some(spawn)) => Ok(self.pools.register(spawn)),
            Ok(None) => return Err(ExportAccountingError::ConflictingSourceDatum.into()),
            Err(error) => match error.downcast::<CatalogError>() {
                Ok(error) => return Err(*error),
                Err(error) => Err(error),
            },
        };
        if let Ok(registered) = &parsed {
            self.registered.insert(fact.ordinal(), *registered);
        }
        Ok(PendingSpawn::read(fact, context.finish(), parsed))
    }

    pub(crate) fn registration(
        &self,
        fact: &OriginalValueFact<'scope, 'read, 'source>,
    ) -> Result<Option<Registered>, CatalogError> {
        self.fields.span(fact)?;
        Ok(self.registered.get(&fact.ordinal()).copied())
    }
    pub(crate) fn emit(
        self,
        members: &TemplateMembers<'fields, 'scope, 'read, 'source>,
    ) -> Result<SpawnEmission<'source>, CatalogError> {
        let mut rows = Vec::with_capacity(self.pending.len());
        let mut failures = Vec::new();
        for pending in self.pending {
            let (row, failure) = pending.emit(members)?;
            rows.push(row);
            failures.extend(failure);
        }
        Ok(SpawnEmission {
            projections: rows,
            pools: self.pools,
            failures,
        })
    }
}
