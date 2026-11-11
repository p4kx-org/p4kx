// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_extraction::{
    PhysicalSourceRef, SourceEvidenceError, SourceScope, SourceUses, StructValue, Value,
};

use super::OriginalFlowFields;
use crate::catalog_read::original::{OriginalContainerKind, OriginalValueEdge, OriginalValueFact};
use crate::{CatalogError, ExportAccountingError};

impl<'scope, 'read, 'source> OriginalFlowFields<'scope, 'read, 'source> {
    pub(crate) fn facts(&self) -> &[OriginalValueFact<'scope, 'read, 'source>] {
        &self.facts
    }

    pub(crate) const fn sources(&self) -> &SourceUses {
        self.scope.sources()
    }

    pub(crate) fn check_context(
        &self,
        sources: &SourceUses,
        dataforge: &SourceScope,
        record_index: u32,
    ) -> Result<(), CatalogError> {
        self.check_cancel()?;
        if !sources.is_same_inventory(self.scope.sources())
            || !std::ptr::eq(dataforge, self.scope.dataforge())
            || record_index != self.record_index()
        {
            return Err(SourceEvidenceError::ForeignEvidence.into());
        }
        Ok(())
    }

    pub(crate) const fn input_ref(&self) -> &PhysicalSourceRef {
        self.scope.dataforge().source_ref()
    }

    /// Captures this exact admitted Record without reconstructing a subject from coordinates.
    pub(crate) fn record_subject(
        &self,
        input: &crate::build::original_input::OriginalCatalogInput<'_, '_>,
    ) -> Result<crate::catalog_read::original::RecordSubject, CatalogError> {
        let context = input.record_context(self.scope.record())?;
        if !std::ptr::eq(context.dataforge(), self.scope.dataforge())
            || !context.sources().is_same_inventory(self.scope.sources())
        {
            return Err(SourceEvidenceError::ForeignEvidence.into());
        }
        crate::catalog_read::original::RecordSubject::capture(input, self.scope.record())
    }
    pub(crate) const fn record_index(&self) -> u32 {
        self.scope.record().index()
    }

    pub(crate) const fn record_guid(&self) -> p4kx_extraction::Guid {
        self.scope.record().guid()
    }

    pub(crate) const fn root(&self) -> &'read StructValue<'source> {
        self.scope.tree().root()
    }

    pub(crate) fn span(
        &self,
        fact: &OriginalValueFact<'scope, 'read, 'source>,
    ) -> Result<Range<usize>, CatalogError> {
        self.check_cancel()?;
        self.ensure(fact)?;
        let start =
            usize::try_from(fact.ordinal()).map_err(|_| ExportAccountingError::CounterOverflow)?;
        let end = self.ends.get(start).copied().flatten().ok_or_else(|| {
            ExportAccountingError::UnfilledLootValue {
                ordinal: fact.ordinal(),
            }
        })?;
        if end <= start || end > self.facts.len() {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        Ok(start..end)
    }

    pub(crate) fn check_tree(
        &self,
        tree: &p4kx_extraction::RecordTree<'source>,
    ) -> Result<(), CatalogError> {
        if !std::ptr::eq(tree, self.scope.tree()) {
            return Err(SourceEvidenceError::ForeignEvidence.into());
        }
        self.check_cancel()
    }

    pub(crate) fn observe_type(
        &self,
        source: &StructValue<'source>,
    ) -> Result<p4kx_extraction::SourceUse, CatalogError> {
        if let Some(fact) = self.owner_of(source)? {
            return fact.observe(
                crate::catalog_read::original::OriginalValuePart::ConcreteType,
                p4kx_extraction::SourceUseRole::Selector,
            );
        }
        Ok(self.scope.dataforge().at(
            crate::record_header_field::RecordHeaderField::SourceType.location(self.record_index()),
            p4kx_extraction::SourceUseRole::Selector,
        )?)
    }

    pub(crate) fn owner_of(
        &self,
        source: &StructValue<'source>,
    ) -> Result<Option<&OriginalValueFact<'scope, 'read, 'source>>, CatalogError> {
        self.check_cancel()?;
        if std::ptr::eq(source, self.root()) {
            return Ok(None);
        }
        let index = self
            .structures
            .get(&std::ptr::from_ref(source))
            .ok_or(SourceEvidenceError::ForeignEvidence)?;
        let fact = self
            .facts
            .get(*index)
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        if !matches!(fact.value(), Value::Struct(original) if std::ptr::eq(original, source)) {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        Ok(Some(fact))
    }

    pub(crate) fn field(
        &self,
        parent: Option<&OriginalValueFact<'scope, 'read, 'source>>,
        name: &str,
    ) -> Result<Option<(u64, &OriginalValueFact<'scope, 'read, 'source>)>, CatalogError> {
        let source = match parent {
            Some(fact) => {
                self.ensure(fact)?;
                let Value::Struct(source) = fact.value() else {
                    return Err(ExportAccountingError::ConflictingSourceDatum.into());
                };
                source
            }
            None => self.root(),
        };
        let owner = parent.map(OriginalValueFact::ordinal);
        let Some(index) = self.first_field(owner, source, name)? else {
            return Ok(None);
        };
        let field = source
            .fields()
            .get(index)
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        let ordinal = u64::try_from(index).map_err(|_| ExportAccountingError::CounterOverflow)?;
        let fact = self.child(
            parent.map(OriginalValueFact::ordinal),
            OriginalValueEdge::Field { ordinal },
        )?;
        if !std::ptr::eq(fact.value(), field.value()) {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        Ok(Some((ordinal, fact)))
    }

    fn first_field(
        &self,
        owner: Option<u64>,
        source: &StructValue<'source>,
        name: &str,
    ) -> Result<Option<usize>, CatalogError> {
        self.check_cancel()?;
        let mut retained = self
            .first_fields
            .try_borrow_mut()
            .map_err(|_| ExportAccountingError::ConflictingSourceDatum)?;
        if let Some(fields) = retained.get(&owner) {
            return Ok(fields.get(name).copied());
        }
        let mut first = std::collections::HashMap::new();
        for (index, field) in source.fields().iter().enumerate() {
            self.check_cancel()?;
            first.entry(field.name()).or_insert(index);
        }
        let selected = first.get(name).copied();
        retained.insert(owner, first);
        Ok(selected)
    }
    fn ensure(&self, fact: &OriginalValueFact<'scope, 'read, 'source>) -> Result<(), CatalogError> {
        if !std::ptr::eq(fact.scope(), self.scope) {
            return Err(SourceEvidenceError::ForeignEvidence.into());
        }
        let index =
            usize::try_from(fact.ordinal()).map_err(|_| ExportAccountingError::CounterOverflow)?;
        let original = self
            .facts
            .get(index)
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        if !std::ptr::eq(fact.value(), original.value())
            || fact.parent_ordinal() != original.parent_ordinal()
            || fact.edge() != original.edge()
        {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        Ok(())
    }

    pub(super) fn end(
        &mut self,
        ordinal: u64,
        kind: OriginalContainerKind,
    ) -> Result<(), CatalogError> {
        let index = usize::try_from(ordinal).map_err(|_| ExportAccountingError::CounterOverflow)?;
        let fact = self
            .facts
            .get(index)
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        if !matches!(
            (kind, fact.value()),
            (OriginalContainerKind::Struct, Value::Struct(_))
                | (OriginalContainerKind::Array, Value::Array(_))
        ) {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        let end = self
            .ends
            .get_mut(index)
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        if end.is_some() {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        *end = Some(self.facts.len());
        Ok(())
    }
}
