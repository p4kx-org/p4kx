// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceUseRole, SourceUses, SourceValue, SourcedValue};

use crate::CatalogError;
use crate::catalog_read::original::{OriginalErrorParameter, OriginalValueFact, OriginalValuePart};

/// Original error facts with creator-specific roles and no target admission authority.
pub(super) struct ErrorFacts<'fact, 'scope, 'read, 'source> {
    pub(super) site: Option<&'fact OriginalValueFact<'scope, 'read, 'source>>,
    pub(super) sources: &'fact SourceUses,
}

impl ErrorFacts<'_, '_, '_, '_> {
    pub(super) fn parameter<T>(
        &self,
        parameter: OriginalErrorParameter,
        value: T,
        role: SourceUseRole,
    ) -> Result<SourcedValue<T>, CatalogError> {
        self.at(OriginalValuePart::Error(parameter), value, role)
    }

    pub(super) fn declaration<T>(&self, value: T) -> Result<SourcedValue<T>, CatalogError> {
        self.at(OriginalValuePart::Target, value, SourceUseRole::Selector)
    }

    pub(super) fn inline_class(&self, value: u32) -> Result<SourcedValue<u32>, CatalogError> {
        if let Some(site) = self.site {
            site.check_inline_class_cycle(value)?;
        }
        self.declaration(value)
    }
    pub(super) fn unbound<T>(&self, value: T) -> SourcedValue<T> {
        self.sources.value(SourceValue::Present(value), &[], false)
    }

    pub(super) fn pointer_struct(&self, value: u32) -> Result<SourcedValue<u32>, CatalogError> {
        self.pointer(OriginalErrorParameter::StructIndex, value)
    }

    pub(super) fn pointer_instance(&self, value: u32) -> Result<SourcedValue<u32>, CatalogError> {
        self.pointer(OriginalErrorParameter::InstanceIndex, value)
    }

    fn pointer(
        &self,
        parameter: OriginalErrorParameter,
        value: u32,
    ) -> Result<SourcedValue<u32>, CatalogError> {
        let Some(site) = self.site else {
            return Ok(self.unbound(value));
        };
        let (part, role) = site.pointer_error_input(parameter, value)?;
        self.at(part, value, role)
    }

    fn at<T>(
        &self,
        part: OriginalValuePart,
        value: T,
        role: SourceUseRole,
    ) -> Result<SourcedValue<T>, CatalogError> {
        let Some(site) = self.site else {
            return Ok(self.unbound(value));
        };
        Ok(site
            .bind(part, SourceValue::Present(value), role, true)?
            .into_cell(self.sources))
    }
}
