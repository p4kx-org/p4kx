// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::collections::hash_map::Entry;

use p4kx_extraction::{Guid, RecordTree, TagPathEdgeKind, TagPathIncoming, Value, ValueError};

use super::{
    ExampleBodyCapture, ExampleBodyReceipt, FieldFailureKind, OriginalFlowFields, RecordSubject,
};
use crate::build::original_input::OriginalCatalogInput;
use crate::{CatalogError, ExportAccountingError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Relation {
    record_index: u32,
    kind: TagPathEdgeKind,
    ordinal: usize,
}

/// Deferred original tag-array occurrences retained while their actual trees remain borrowed.
#[derive(Default)]
pub(crate) struct TagRelationReceipts {
    relations: HashMap<Relation, ExampleBodyReceipt>,
}

impl TagRelationReceipts {
    pub(crate) fn capture<'scope, 'read, 'source>(
        &mut self,
        kind: TagPathEdgeKind,
        tree: &RecordTree<'source>,
        fields: &OriginalFlowFields<'scope, 'read, 'source>,
        body: &ExampleBodyCapture<'scope, 'read, 'source>,
    ) -> Result<(), CatalogError> {
        fields.check_tree(tree)?;
        let Some(Value::Array(items)) = fields.root().field(kind.field()) else {
            return Ok(());
        };
        for (ordinal, value) in items.iter().enumerate() {
            fields.check_cancel()?;
            if matches!(value, Value::Null) {
                continue;
            }
            let fact = fields.fact(value)?;
            let ancestors = fields.ancestors(fact)?;
            let receipt = match value {
                Value::Reference(_) | Value::Error(ValueError::DanglingReference { .. }) => {
                    body.reference_fact(fields.root(), fact, &ancestors, kind.field())?
                }
                _ => body.field_fact(
                    fields.root(),
                    fact,
                    &ancestors,
                    kind.field(),
                    FieldFailureKind::Invalid,
                )?,
            };
            let relation = Relation {
                record_index: fields.record_index(),
                kind,
                ordinal,
            };
            match self.relations.entry(relation) {
                Entry::Vacant(slot) => {
                    slot.insert(receipt);
                }
                Entry::Occupied(_) => {
                    return Err(ExportAccountingError::ConflictingSourceDatum.into());
                }
            }
        }
        Ok(())
    }

    pub(crate) fn take(
        &mut self,
        input: &OriginalCatalogInput<'_, '_>,
        subject: &RecordSubject,
        incoming: TagPathIncoming<'_, '_>,
        target: Option<Guid>,
    ) -> Result<ExampleBodyReceipt, CatalogError> {
        input.record_context(incoming.record())?;
        subject.check_input(input)?;
        let relation = Relation {
            record_index: incoming.record().index(),
            kind: incoming.kind(),
            ordinal: incoming.ordinal(),
        };
        let receipt = self
            .relations
            .get(&relation)
            .ok_or(ExportAccountingError::UnboundHealthSubject)?;
        receipt.check_input(input)?;
        receipt.check_record_owner(subject)?;
        let (field, _, actual) = receipt.description()?;
        if field.as_deref() != Some(incoming.kind().field()) || actual != target {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        self.relations
            .remove(&relation)
            .ok_or_else(|| ExportAccountingError::ConflictingSourceDatum.into())
    }
}

#[cfg(test)]
#[path = "health_tag_relation_tests.rs"]
mod tests;
