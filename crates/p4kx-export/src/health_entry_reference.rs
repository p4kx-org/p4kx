// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{PhysicalSourceRef, SourceUse, SourceUseRole, SourceValue, SourcedValue};

use super::{
    AuthoredEntryPath, EntryCapture, EntryMetadata, EntryReceipt, OriginalDatum, PathRecipe,
};
use crate::{CatalogError, ExportAccountingError, HealthText};

/// Exact directory identity and path facts for one observed physical Archive Entry.
#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = crate::schema_fields::every_field_required)]
pub struct PhysicalEntryIdentity {
    pub(crate) input_ref: PhysicalSourceRef,
    pub(crate) entry_index: u32,
    pub(crate) source_path: SourcedValue<String>,
    pub(crate) normalized_path: SourcedValue<String>,
}

impl PhysicalEntryIdentity {
    /// Borrows the report-local physical input identity.
    ///
    /// Resolve it through the source inventory in Full provenance.
    #[must_use]
    pub const fn input_ref(&self) -> &PhysicalSourceRef {
        &self.input_ref
    }

    /// Returns the physical directory ordinal.
    #[must_use]
    pub const fn entry_index(&self) -> u32 {
        self.entry_index
    }

    /// Borrows the sourced path with its original directory spelling.
    #[must_use]
    pub const fn source_path(&self) -> &SourcedValue<String> {
        &self.source_path
    }

    /// Borrows the sourced normalized path.
    #[must_use]
    pub const fn normalized_path(&self) -> &SourcedValue<String> {
        &self.normalized_path
    }
}

/// Structural navigation to one actual physical Archive Entry.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = crate::schema_fields::every_field_required)]
pub struct PhysicalEntryRef {
    pub(crate) input_ref: PhysicalSourceRef,
    pub(crate) entry_index: u32,
}

impl PhysicalEntryRef {
    /// Returns the observed Archive input identity of this Entry.
    #[must_use]
    pub const fn input_ref(&self) -> &PhysicalSourceRef {
        &self.input_ref
    }

    /// Returns its physical directory ordinal.
    #[must_use]
    pub const fn entry_index(&self) -> u32 {
        self.entry_index
    }
}

impl EntryReceipt {
    pub(crate) fn physical_file(
        &self,
        normalize: impl FnOnce(&str) -> String,
    ) -> Result<PhysicalEntryIdentity, CatalogError> {
        self.describe_file(self.outer_directory()?, normalize)
    }

    pub(crate) fn physical_payload_file(
        &self,
        normalize: impl FnOnce(&str) -> String,
    ) -> Result<PhysicalEntryIdentity, CatalogError> {
        self.describe_file(self.payload_entry()?, normalize)
    }

    fn describe_file(
        &self,
        origin: &EntryMetadata,
        normalize: impl FnOnce(&str) -> String,
    ) -> Result<PhysicalEntryIdentity, CatalogError> {
        let sources = self.source_uses();
        let path = origin.path().as_str();
        let raw = origin.metadata("path", SourceUseRole::Payload)?;
        let normalized = origin.metadata("path", SourceUseRole::Conversion)?;
        Ok(PhysicalEntryIdentity {
            input_ref: origin.source_ref().clone(),
            entry_index: origin.index(),
            source_path: sources.value(SourceValue::Present(path.to_owned()), &[raw], true),
            normalized_path: sources.value(
                SourceValue::Present(normalize(path)),
                &[normalized],
                true,
            ),
        })
    }

    pub(crate) fn physical_path(&self) -> Result<OriginalDatum<HealthText>, CatalogError> {
        self.outer_directory()?;
        self.directory_path()
    }

    /// The exact outer Entry path used as a directory lookup selector.
    pub(crate) fn directory_path_selector(&self) -> Result<SourceUse, CatalogError> {
        self.outer_directory()?
            .metadata("path", SourceUseRole::Selector)
    }

    fn outer_directory(&self) -> Result<&EntryMetadata, ExportAccountingError> {
        if !matches!(self.recipes.as_slice(), [PathRecipe::Directory]) || self.inner.is_some() {
            return Err(ExportAccountingError::ConflictingSourceDatum);
        }
        self.outer
            .as_ref()
            .ok_or(ExportAccountingError::ConflictingSourceDatum)
    }

    pub(crate) fn add_authored_selector(
        &mut self,
        selector: &AuthoredEntryPath,
    ) -> Result<(), ExportAccountingError> {
        self.frame.check_frame(&selector.frame())?;
        self.recipes.push(PathRecipe::Authored(selector.clone()));
        Ok(())
    }

    pub(crate) fn physical_reference(
        &self,
        capture: &EntryCapture<'_>,
    ) -> Result<OriginalDatum<PhysicalEntryRef>, CatalogError> {
        self.check_capture(capture)?;
        Self::reference_of(self.outer_directory()?)
    }

    pub(crate) fn payload_reference(
        &self,
        capture: &EntryCapture<'_>,
    ) -> Result<OriginalDatum<PhysicalEntryRef>, CatalogError> {
        self.check_capture(capture)?;
        Self::reference_of(self.payload_entry()?)
    }

    fn reference_of(
        origin: &EntryMetadata,
    ) -> Result<OriginalDatum<PhysicalEntryRef>, CatalogError> {
        let value = PhysicalEntryRef {
            input_ref: origin.source_ref().clone(),
            entry_index: origin.index(),
        };
        let uses = vec![
            origin.metadata("path", SourceUseRole::Selector)?,
            origin.metadata("entryIndex", SourceUseRole::Selector)?,
        ];
        Ok(OriginalDatum::observed(
            SourceValue::Present(value),
            uses,
            true,
        ))
    }
}
