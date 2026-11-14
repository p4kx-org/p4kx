// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_extraction::SourceUse;

use super::OriginalDatum;
use super::authored_entry_text::AuthoredEntryText;
use super::health_entry_frame::{EntryCapture, EntryFrame};
use crate::build::original_input::OriginalCatalogInput;
use crate::{CatalogError, ExportAccountingError, HealthText};

use super::entry_selection_input::EntrySelectionUse;

/// A path paired at its actual original parse or token selection.
#[derive(Debug, Clone)]
pub(crate) struct AuthoredEntryPath {
    inner: AuthoredEntryText,
}

impl AuthoredEntryPath {
    pub(super) fn captured(
        capture: &EntryCapture<'_>,
        text: &str,
        inputs: Vec<EntrySelectionUse>,
    ) -> Result<Self, CatalogError> {
        Ok(Self::from_text(AuthoredEntryText::captured(
            capture, text, inputs,
        )?))
    }
    pub(super) const fn from_text(inner: AuthoredEntryText) -> Self {
        Self { inner }
    }
    pub(super) fn with_xml_operation(
        mut self,
        operation: &p4kx_assembly::LoadoutFileObservation,
    ) -> Self {
        self.inner = self.inner.with_xml_operation(operation);
        self
    }
    pub(crate) fn is_from_xml(&self, operation: &p4kx_assembly::LoadoutFileObservation) -> bool {
        self.inner.is_from_xml(operation)
    }
    pub(crate) fn is_same_recipe(&self, other: &Self) -> bool {
        self.inner.is_same_recipe(&other.inner)
    }
    pub(super) fn frame(&self) -> Arc<EntryFrame> {
        self.inner.frame()
    }
    pub(crate) fn as_str(&self) -> &str {
        self.inner.as_str()
    }
    pub(super) const fn text(&self) -> &HealthText {
        self.inner.text()
    }
    pub(crate) fn derived(
        capture: &EntryCapture<'_>,
        inputs: &[&Self],
        value: &str,
    ) -> Result<Self, CatalogError> {
        Ok(Self::from_text(AuthoredEntryText::derived(
            capture,
            inputs.iter().map(|input| &input.inner),
            value,
        )?))
    }
    pub(crate) fn selector_inputs(&self) -> Result<(Vec<SourceUse>, bool), CatalogError> {
        self.inner.selector_inputs()
    }
    pub(crate) fn convert(
        self,
        capture: &EntryCapture<'_>,
        convert: impl FnOnce(&str) -> Option<String>,
    ) -> Result<Option<Self>, CatalogError> {
        Ok(self.inner.convert(capture, convert)?.map(Self::from_text))
    }
    pub(crate) fn selected_by(self, selector: &Self) -> Result<Self, ExportAccountingError> {
        Ok(Self::from_text(self.inner.selected_by(&selector.inner)?))
    }
    pub(crate) fn check_input(
        &self,
        input: &OriginalCatalogInput<'_, '_>,
    ) -> Result<(), ExportAccountingError> {
        self.inner.check_input(input)
    }
    pub(crate) fn retain_in(
        &self,
        capture: &EntryCapture<'_>,
    ) -> Result<OriginalDatum<HealthText>, CatalogError> {
        self.frame().check_frame(&capture.frame())?;
        self.retain()
    }

    pub(super) fn retain(&self) -> Result<OriginalDatum<HealthText>, CatalogError> {
        self.inner.retain()
    }
}

#[cfg(test)]
#[path = "health_entry_record_selection_tests.rs"]
mod record_selection_tests;
