// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_extraction::{RecordTree, SourceLocation, SourceUseRole, StructValue, Value};

use super::authored_entry_text::AuthoredEntryText;
use super::entry_selection_input::{EntrySelectionUse, SelectionSource};
use super::health_entry_frame::EntryCapture;
use super::health_entry_selection::AuthoredEntryPath;
use super::health_reference_receipt::ReferenceFrame;
use super::{OriginalRecordScope, OriginalValueEdge, OriginalValueFact, OriginalValuePart};
use crate::build::original_input::OriginalCatalogInput;
use crate::record_header_field::RecordHeaderField;
use crate::record_observation::RecordObservation;
use crate::{CatalogError, ExportAccountingError};

/// The actual Record read borrowed while its real path getter is evaluated.
pub(crate) struct EntryPathCapture<'scope, 'read, 'source> {
    original_scope: &'scope OriginalRecordScope<'read, 'source>,
    record: Arc<ReferenceFrame>,
    entries: EntryCapture<'scope>,
}

impl<'scope, 'read, 'source> EntryPathCapture<'scope, 'read, 'source> {
    pub(crate) fn new(
        input: &'scope OriginalCatalogInput<'_, '_>,
        admission: &RecordObservation<'_, '_>,
        scope: &'scope OriginalRecordScope<'read, 'source>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            original_scope: scope,
            record: Arc::new(ReferenceFrame::read(input, admission, scope)?),
            entries: EntryCapture::for_input(input),
        })
    }

    pub(super) const fn scope(&self) -> &'scope OriginalRecordScope<'read, 'source> {
        self.original_scope
    }

    pub(super) fn reference_frame(&self) -> Arc<ReferenceFrame> {
        Arc::clone(&self.record)
    }

    pub(crate) fn value_fact(
        &self,
        parent: &StructValue<'source>,
        fact: &OriginalValueFact<'scope, 'read, 'source>,
        ancestors: &[&OriginalValueFact<'scope, 'read, 'source>],
    ) -> Result<AuthoredEntryPath, CatalogError> {
        Ok(AuthoredEntryPath::from_text(
            self.text_fact(parent, fact, ancestors)?,
        ))
    }

    pub(super) fn text_fact(
        &self,
        parent: &StructValue<'source>,
        fact: &OriginalValueFact<'scope, 'read, 'source>,
        ancestors: &[&OriginalValueFact<'scope, 'read, 'source>],
    ) -> Result<AuthoredEntryText, CatalogError> {
        if !std::ptr::eq(fact.scope(), self.original_scope) {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        super::health_value_chain::parent(parent, fact, ancestors)?;
        let (Value::String(value) | Value::Locale(value) | Value::Enum(value)) = fact.value()
        else {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        };
        let mut inputs = vec![self.input(
            fact.location(OriginalValuePart::Value),
            SourceUseRole::Payload,
        )];
        inputs.push(self.input(
            RecordHeaderField::SourceType.location(fact.scope().record().index()),
            SourceUseRole::Selector,
        ));
        for ancestor in ancestors.iter().copied().chain(std::iter::once(fact)) {
            if matches!(ancestor.edge(), OriginalValueEdge::Field { .. }) {
                inputs.push(self.input(
                    ancestor.location(OriginalValuePart::FieldName),
                    SourceUseRole::Selector,
                ));
            }
            let part = match ancestor.value() {
                Value::Struct(_) => Some(OriginalValuePart::ConcreteType),
                Value::Array(_) => Some(OriginalValuePart::Value),
                _ => None,
            };
            if let Some(part) = part {
                inputs.push(self.input(ancestor.location(part), SourceUseRole::Selector));
            }
        }
        AuthoredEntryText::captured(&self.entries, value, inputs)
    }

    pub(crate) fn record_name(
        &self,
        tree: &RecordTree<'_>,
    ) -> Result<AuthoredEntryPath, CatalogError> {
        if !std::ptr::addr_eq(tree, self.original_scope.tree()) {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        AuthoredEntryPath::captured(
            &self.entries,
            tree.name(),
            vec![self.input(
                RecordHeaderField::RecordName.location(self.original_scope.record().index()),
                SourceUseRole::Payload,
            )],
        )
    }

    fn input(&self, location: SourceLocation, role: SourceUseRole) -> EntrySelectionUse {
        EntrySelectionUse {
            source: SelectionSource::Record(Arc::clone(&self.record)),
            location,
            role,
        }
    }
}
