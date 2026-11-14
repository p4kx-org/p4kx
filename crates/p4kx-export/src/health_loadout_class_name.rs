// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{LoadoutDocumentKind, LoadoutFileObservation, XmlLoadoutClassSite};
use p4kx_extraction::{SourceUse, StructValue, Value};

use super::authored_entry_text::AuthoredEntryText;
use super::health_entry_frame::EntryCapture;
use super::health_loadout_graph::ClassXmlSelection;
use super::{EntryPathCapture, OriginalValueEdge, OriginalValueFact};
use crate::build::original_input::OriginalCatalogInput;
use crate::{CatalogError, ExportAccountingError};

/// A class name captured only at its actual Record or same-parse XML getter.
#[derive(Debug, Clone)]
pub(crate) struct LoadoutClassNameReceipt {
    text: AuthoredEntryText,
    xml: Option<XmlClassName>,
}

#[derive(Debug, Clone)]
struct XmlClassName {
    operation: LoadoutFileObservation,
    selection: Option<ClassXmlSelection>,
}

impl LoadoutClassNameReceipt {
    pub(crate) fn record<'scope, 'read, 'source>(
        capture: &EntryPathCapture<'scope, 'read, 'source>,
        parent: &StructValue<'source>,
        fact: &OriginalValueFact<'scope, 'read, 'source>,
        ancestors: &[&OriginalValueFact<'scope, 'read, 'source>],
    ) -> Result<Self, CatalogError> {
        if !matches!(fact.edge(), OriginalValueEdge::Field { .. })
            || fact.field().name() != "entityClassName"
            || !matches!(fact.value(), Value::String(value) | Value::Locale(value) | Value::Enum(value) if !value.is_empty())
        {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        Ok(Self {
            text: capture.text_fact(parent, fact, ancestors)?,
            xml: None,
        })
    }

    pub(crate) fn xml(
        input: &OriginalCatalogInput<'_, '_>,
        site: &XmlLoadoutClassSite<'_>,
    ) -> Result<Self, CatalogError> {
        if site.attribute_name() != "itemName"
            || site.value().is_empty()
            || site.observation().kind() != LoadoutDocumentKind::XmlLoadout
        {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        let text = EntryCapture::for_input(input).loadout_class_text(site)?;
        Ok(Self {
            text,
            xml: Some(XmlClassName {
                operation: site.observation().clone(),
                selection: None,
            }),
        })
    }

    pub(super) fn original_xml(&self) -> Option<&LoadoutFileObservation> {
        self.xml.as_ref().map(|xml| &xml.operation)
    }

    pub(super) fn bind_xml(
        &mut self,
        selection: ClassXmlSelection,
    ) -> Result<(), ExportAccountingError> {
        let xml = self
            .xml
            .as_mut()
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        if xml.selection.is_some() || !selection.belongs_to(&xml.operation) {
            return Err(ExportAccountingError::ConflictingSourceDatum);
        }
        xml.selection = Some(selection);
        Ok(())
    }

    pub(super) fn value(&self) -> &str {
        self.text.as_str()
    }

    pub(super) fn check_input(
        &self,
        input: &OriginalCatalogInput<'_, '_>,
    ) -> Result<(), ExportAccountingError> {
        self.text.check_input(input)?;
        if let Some(selectors) = self
            .xml
            .as_ref()
            .and_then(|xml| xml.selection.as_ref())
            .map(ClassXmlSelection::paths)
        {
            for selector in selectors {
                selector.check_input(input)?;
            }
        }
        Ok(())
    }

    pub(super) fn selectors(&self) -> Result<(Vec<SourceUse>, bool), CatalogError> {
        let (mut inputs, mut complete) = self.text.selector_inputs()?;
        if let Some(xml) = &self.xml {
            let paths = xml
                .selection
                .as_ref()
                .ok_or(ExportAccountingError::UnboundHealthSubject)?
                .paths();
            for path in paths {
                let (selected, known) = path.selector_inputs()?;
                inputs.extend(selected);
                complete &= known;
            }
        }
        Ok((inputs, complete))
    }
}
