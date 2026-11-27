// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::{ResourceFloat32, SourceRecordReference};
use p4kx_extraction::{FieldOrigin, SourceEncryptionState, SourceInventory, SourcedValue};
use serde::Serialize;
use serde_json::{Value, json};

use super::TestResult;

pub(super) struct View<'a> {
    inventory: &'a SourceInventory,
    pub(super) cells: usize,
}

impl<'a> View<'a> {
    pub(super) const fn new(inventory: &'a SourceInventory) -> Self {
        Self {
            inventory,
            cells: 0,
        }
    }

    pub(super) fn cell<T>(
        &mut self,
        cell: &SourcedValue<T>,
        project: impl FnOnce(&mut Self, &T) -> TestResult<Value>,
    ) -> TestResult<Value> {
        self.check_origin(cell.origin())?;
        self.cells += 1;
        let value = match cell.value() {
            Some(value) => project(self, value)?,
            None => Value::Null,
        };
        Ok(json!({
            "state": cell.state(), "value": value,
            "containsEncryptedSource": cell.origin().contains_encrypted_source(),
            "sourceSetComplete": cell.origin().source_set_complete(),
            "sourceUsesRefs": cell.origin().source_uses_refs(),
        }))
    }

    pub(super) fn scalar<T: Serialize>(&mut self, value: &SourcedValue<T>) -> TestResult<Value> {
        self.cell(value, |_, value| Ok(serde_json::to_value(value)?))
    }

    pub(super) fn sequence<T>(
        &mut self,
        cell: &SourcedValue<Vec<SourcedValue<T>>>,
        project: impl Fn(&mut Self, &T) -> TestResult<Value>,
    ) -> TestResult<Value> {
        self.cell(cell, |view, rows| {
            rows.iter()
                .map(|row| view.cell(row, &project))
                .collect::<TestResult<Vec<_>>>()
                .map(Value::Array)
        })
    }

    pub(super) fn reference(&mut self, value: &SourceRecordReference<'_>) -> TestResult<Value> {
        let guid = value.guid();
        Ok(json!({
            "guid": self.cell(&guid, |_, guid| Ok(json!(guid.to_string())))?,
            "targetRecordIndex": self.scalar(value.target_record_index())?,
            "recordPath": self.scalar(value.record_path())?,
            "targetType": self.scalar(value.target_type())?,
            "status": value.status(),
        }))
    }

    pub(super) fn float(&mut self, value: &SourcedValue<ResourceFloat32>) -> TestResult<Value> {
        self.cell(value, |_, value| {
            Ok(json!({
                "rawBits": format!("{:08x}", value.raw_bits()), "finite": value.finite(),
            }))
        })
    }

    fn check_origin(&self, origin: &FieldOrigin) -> TestResult {
        let mut encrypted = false;
        let mut unknown = !origin.source_set_complete();
        for reference in origin.source_uses_refs() {
            let used = self
                .inventory
                .iter_uses()
                .find(|used| used.id() == reference)
                .ok_or("sourced cell use must be in the same query inventory")?;
            let source = self
                .inventory
                .iter_sources()
                .find(|source| source.id() == used.source())
                .ok_or("use must resolve to its physical source")?;
            let _facts = (
                used.scope(),
                used.role(),
                used.location(),
                source.kind(),
                source.stored_encryption(),
            );
            for ancestor in source.ancestors() {
                assert!(
                    self.inventory
                        .iter_sources()
                        .any(|source| source.id() == ancestor),
                    "physical source ancestry stays in the same inventory"
                );
            }
            match used.effective_encryption() {
                SourceEncryptionState::Encrypted => encrypted = true,
                SourceEncryptionState::Unencrypted => {}
                _ => unknown = true,
            }
        }
        let expected = if encrypted {
            Some(true)
        } else if unknown {
            None
        } else {
            Some(false)
        };
        assert_eq!(
            origin.contains_encrypted_source(),
            expected,
            "each datum preserves the actual contributor encryption state"
        );
        Ok(())
    }
}
