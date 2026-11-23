// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceUse, SourceUses, SourceValue, SourcedValue};

use super::{Class, Found};
use crate::CatalogError;
use crate::catalog_read::original::AuthoredEntryPath;

pub(crate) struct ShopClassCells {
    pub(crate) kind: SourcedValue<String>,
    pub(crate) brand: SourcedValue<String>,
}

pub(super) struct ClassSurvey<'read> {
    pub(super) directory: Option<&'read SourceUse>,
    pub(super) complete: bool,
    pub(super) classified: bool,
}

#[derive(Debug, Default)]
pub(super) struct ClassEvidence {
    classes: Vec<Class>,
    inputs: Vec<SourceUse>,
    incomplete: bool,
}

impl ClassEvidence {
    pub(super) fn observe(
        &mut self,
        class: &Class,
        path: &SourceUse,
        guids: &[AuthoredEntryPath],
    ) -> Result<(), CatalogError> {
        if !self.classes.contains(class) {
            self.classes.push(class.clone());
        }
        append(&mut self.inputs, std::slice::from_ref(path))?;
        for guid in guids {
            let (inputs, complete) = guid.selector_inputs()?;
            append(&mut self.inputs, &inputs)?;
            self.incomplete |= !complete;
        }
        Ok(())
    }

    pub(super) const fn found(&self) -> Found<'_> {
        match self.classes.as_slice() {
            [] => Found::Nowhere,
            [class] => Found::One(class),
            _ => Found::Mixed,
        }
    }

    pub(super) fn cells(
        &self,
        selector: Option<&AuthoredEntryPath>,
        survey: &ClassSurvey<'_>,
        sources: &SourceUses,
    ) -> Result<ShopClassCells, CatalogError> {
        let (mut inputs, selector_complete) = match selector {
            Some(selector) => selector.selector_inputs()?,
            None => (Vec::new(), false),
        };
        append(&mut inputs, &self.inputs)?;
        if let Some(directory) = survey.directory {
            append(&mut inputs, std::slice::from_ref(directory))?;
        }
        let complete = survey.complete && selector_complete && !self.incomplete;
        let missing = || {
            if complete {
                SourceValue::Absent
            } else {
                SourceValue::Unreadable
            }
        };
        let found = self.found();
        let retained_complete =
            complete && (!survey.classified || !matches!(found, Found::Nowhere));
        let (kind, brand) = match found {
            Found::One(class) => (
                SourceValue::Present(class.kind.clone()),
                class
                    .brand
                    .clone()
                    .map_or_else(missing, SourceValue::Present),
            ),
            Found::Mixed => (SourceValue::Invalid, SourceValue::Invalid),
            Found::Nowhere => (missing(), missing()),
        };
        Ok(ShopClassCells {
            kind: sources.value(kind, &inputs, retained_complete),
            brand: sources.value(brand, &inputs, retained_complete),
        })
    }
}

fn append(uses: &mut Vec<SourceUse>, added: &[SourceUse]) -> Result<(), CatalogError> {
    uses.try_reserve(added.len())
        .map_err(CatalogError::SourceUseAllocation)?;
    uses.extend_from_slice(added);
    Ok(())
}
