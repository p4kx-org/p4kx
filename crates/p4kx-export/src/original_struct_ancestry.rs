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
        if storage.data_type() != DataType::StrongPointer {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        let (_, declaration) = self.observe_declared_target(input)?;
        selectors.extend(declaration);
        self.descends_from_any(&[u32::from(storage.target())], selectors, check_cancel)
    }

    pub(crate) fn descends_from_any(
        &self,
        expected: &[u32],
        selectors: &mut Vec<SourceUse>,
        check_cancel: impl Fn() -> Result<(), CatalogError>,
    ) -> Result<bool, CatalogError> {
        let Value::Struct(source) = self.value() else {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        };
        let current = self
            .scope()
            .struct_at(source.struct_index())
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        self.observe_ancestry(
            current,
            |index, _| Ok(expected.contains(&index)),
            selectors,
            check_cancel,
        )
    }

    pub(crate) fn descends_from_any_named(
        &self,
        input: &OriginalCatalogInput<'_, '_>,
        names: &[&str],
        selectors: &mut Vec<SourceUse>,
        check_cancel: impl Fn() -> Result<(), CatalogError>,
    ) -> Result<Option<bool>, CatalogError> {
        self.check_input(input)?;
        let Value::Struct(source) = self.value() else {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        };
        let index = source.struct_index();
        let current = self
            .scope()
            .struct_at(index)
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        let mut readable = true;
        let found = self.observe_ancestry(
            current,
            |index, selectors| {
                let Some(structure) = self.scope().struct_at(index) else {
                    return Err(ExportAccountingError::ConflictingSourceDatum.into());
                };
                let name = structure.name();
                let role = if name.is_ok() {
                    SourceUseRole::Selector
                } else {
                    SourceUseRole::Attempted
                };
                selectors.push(self.scope().dataforge().at(
                    SourceLocation::Subresource {
                        namespace: "dataforgeStruct".to_owned(),
                        indices: vec![u64::from(index)],
                        field: "name".to_owned(),
                    },
                    role,
                )?);
                Ok(name.map_or_else(
                    |_| {
                        readable = false;
                        true
                    },
                    |name| names.contains(&name),
                ))
            },
            selectors,
            check_cancel,
        )?;
        Ok(readable.then_some(found))
    }

    pub(super) fn observe_struct_ancestry(
        &self,
        current: Struct<'_>,
        expected: u32,
        selectors: &mut Vec<SourceUse>,
        check_cancel: impl Fn() -> Result<(), CatalogError>,
    ) -> Result<bool, CatalogError> {
        self.observe_ancestry(
            current,
            |index, _| Ok(index == expected),
            selectors,
            check_cancel,
        )
    }

    fn observe_ancestry(
        &self,
        mut current: Struct<'_>,
        mut matches: impl FnMut(u32, &mut Vec<SourceUse>) -> Result<bool, CatalogError>,
        selectors: &mut Vec<SourceUse>,
        check_cancel: impl Fn() -> Result<(), CatalogError>,
    ) -> Result<bool, CatalogError> {
        let scope = self.scope().dataforge();
        let start = current.index();
        let mut seen = HashSet::new();
        loop {
            check_cancel()?;
            let index = current.index();
            if matches(index, selectors)? {
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
