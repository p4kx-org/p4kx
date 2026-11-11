// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    DataForge, DataForgeError, Record, RecordTree, SourceEvidenceError, SourceLocation, SourceUse,
    SourceUseRole, Struct, ValueError,
};

use crate::CatalogError;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_read::CatalogRecordContext;

use super::OriginalRecordScope;

/// The original opened DataForge paired with the same build's physical source scopes.
pub(crate) struct OriginalCatalogInput<'read, 'source> {
    forge: &'read DataForge<'source>,
    origins: &'read CatalogOrigins,
}

impl<'read, 'source> OriginalCatalogInput<'read, 'source> {
    pub(in crate::build) const fn new(
        forge: &'read DataForge<'source>,
        origins: &'read CatalogOrigins,
    ) -> Self {
        Self { forge, origins }
    }

    pub(crate) const fn origins(&self) -> &'read CatalogOrigins {
        self.origins
    }

    /// Rejects a Forge other than the original admitted input.
    pub(crate) fn check_forge(&self, forge: &DataForge<'_>) -> Result<(), CatalogError> {
        if !std::ptr::addr_eq(self.forge, forge) {
            return Err(SourceEvidenceError::ForeignEvidence.into());
        }
        Ok(())
    }
    /// Admits original header facts without requiring a successful body read.
    pub(crate) fn record_context(
        &self,
        record: &Record<'_>,
    ) -> Result<CatalogRecordContext<'_>, CatalogError> {
        self.check_record(record)?;
        Ok(self.origins.record(record.index()))
    }

    /// Binds one actual canonical read before any field occurrence is observed.
    pub(crate) fn record_scope<'scope, 'tree>(
        &'scope self,
        record: &'scope Record<'tree>,
        tree: &'scope RecordTree<'tree>,
    ) -> Result<OriginalRecordScope<'scope, 'tree>, CatalogError> {
        OriginalRecordScope::new(self, record, tree)
    }

    /// Reads the actual name of a declared original Struct target.
    pub(crate) fn declared_struct_name(
        &self,
        index: u16,
        role: SourceUseRole,
    ) -> Result<(Result<&str, ValueError>, SourceUse), CatalogError> {
        let name = match self.struct_at(u32::from(index)) {
            Some(declaration) => declaration_name(declaration.name())?,
            None => Err(ValueError::UnknownStruct { index }),
        };
        let used = self.declaration_name_use(index, "dataforgeStruct", role, name.is_ok())?;
        Ok((name, used))
    }

    /// Reads the actual name of a declared original Enum target.
    pub(crate) fn declared_enum_name(
        &self,
        index: u16,
    ) -> Result<(Result<&str, ValueError>, SourceUse), CatalogError> {
        let name = match self.forge.enums().nth(usize::from(index)) {
            Some(declaration) => declaration_name(declaration.name())?,
            None => Err(ValueError::UnknownEnum { index }),
        };
        let used = self.declaration_name_use(
            index,
            "dataforgeEnum",
            SourceUseRole::Selector,
            name.is_ok(),
        )?;
        Ok((name, used))
    }

    fn declaration_name_use(
        &self,
        index: u16,
        namespace: &str,
        role: SourceUseRole,
        readable: bool,
    ) -> Result<SourceUse, CatalogError> {
        let role = if readable {
            role
        } else {
            SourceUseRole::Attempted
        };
        Ok(self.origins.dataforge.at(
            SourceLocation::Subresource {
                namespace: namespace.to_owned(),
                indices: vec![u64::from(index)],
                field: "name".to_owned(),
            },
            role,
        )?)
    }

    pub(super) fn struct_name(&self, index: u16) -> Option<&str> {
        self.struct_at(u32::from(index))
            .and_then(|structure| structure.name().ok())
    }

    pub(super) fn struct_at(&self, index: u32) -> Option<Struct<'_>> {
        self.forge.struct_at(index)
    }

    fn check_record(&self, record: &Record<'_>) -> Result<(), CatalogError> {
        if !record.is_from(self.forge) {
            return Err(SourceEvidenceError::ForeignEvidence.into());
        }
        Ok(())
    }
}

const fn declaration_name(
    name: Result<&str, DataForgeError>,
) -> Result<Result<&str, ValueError>, CatalogError> {
    match name {
        Ok(name) => Ok(Ok(name)),
        Err(DataForgeError::InvalidString { table, offset }) => {
            Ok(Err(ValueError::InvalidString { table, offset }))
        }
        Err(error) => Err(CatalogError::DataForge(error)),
    }
}
