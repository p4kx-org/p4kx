// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;

use p4kx_extraction::{
    DataForgeError, DataType, SourceLocation, SourceUse, SourceUseRole, Struct, Value,
};

use super::OriginalValueFact;
use crate::build::original_input::OriginalCatalogInput;
use crate::{CatalogError, ExportAccountingError};

impl OriginalValueFact<'_, '_, '_> {
    /// Checks the actual owned target against the field's physical `StrongPointer` declaration.
    pub(crate) fn matches_owned_declaration(
        &self,
        input: &OriginalCatalogInput<'_, '_>,
        selectors: &mut Vec<SourceUse>,
        check_cancel: impl Fn() -> Result<(), CatalogError>,
    ) -> Result<bool, CatalogError> {
        let storage = self.field().storage();
        let Value::Struct(source) = self.value() else {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        };
        if storage.data_type() != DataType::StrongPointer {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        let (_, declaration) = self.observe_declared_target(input)?;
        selectors.extend(declaration);
        let current = self
            .scope()
            .struct_at(source.struct_index())
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        self.observe_struct_ancestry(
            current,
            u32::from(storage.target()),
            selectors,
            check_cancel,
        )
    }

    pub(super) fn observe_struct_ancestry(
        &self,
        mut current: Struct<'_>,
        expected: u32,
        selectors: &mut Vec<SourceUse>,
        check_cancel: impl Fn() -> Result<(), CatalogError>,
    ) -> Result<bool, CatalogError> {
        let scope = self.scope().dataforge();
        let start = current.index();
        let mut seen = HashSet::new();
        loop {
            check_cancel()?;
            let index = current.index();
            if index == expected {
                return Ok(true);
            }
            if !seen.insert(index) {
                return Err(CatalogError::DataForge(DataForgeError::StructCycle {
                    struct_index: start,
                }));
            }
            selectors.push(scope.at(
                SourceLocation::Subresource {
                    namespace: "dataforgeStructDeclaration".to_owned(),
                    indices: vec![u64::from(index)],
                    field: "parent".to_owned(),
                },
                SourceUseRole::Selector,
            )?);
            let Some(parent) = current.parent() else {
                return Ok(false);
            };
            current = parent;
        }
    }
}
