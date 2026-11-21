// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{
    LoadoutFileObservation, LoadoutPathOperation, XmlLoadoutClassSite, XmlLoadoutPathSite,
};
use p4kx_extraction::{SourceLocation, SourceUseRole};

use super::authored_entry_text::AuthoredEntryText;
use super::entry_selection_input::{EntrySelectionUse, SelectionSource};
use super::health_entry_frame::EntryCapture;
use super::health_entry_selection::AuthoredEntryPath;
use crate::{CatalogError, ExportAccountingError};

enum XmlEntrySite<'site, 'parse> {
    Path(&'site XmlLoadoutPathSite<'parse>),
    Class(&'site XmlLoadoutClassSite<'parse>),
}

struct XmlDatum<'site> {
    operation: &'site LoadoutFileObservation,
    node: u32,
    attribute: Option<u64>,
    name: &'site str,
    value: &'site str,
}

impl XmlEntrySite<'_, '_> {
    const fn datum(&self) -> XmlDatum<'_> {
        match self {
            Self::Path(site) => XmlDatum {
                operation: site.observation(),
                node: site.node_index(),
                attribute: if matches!(site.operation(), LoadoutPathOperation::PortName) {
                    Some(site.attribute_index())
                } else {
                    None
                },
                name: site.attribute_name(),
                value: site.value(),
            },
            Self::Class(site) => XmlDatum {
                operation: site.observation(),
                node: site.node_index(),
                attribute: Some(site.attribute_index()),
                name: site.attribute_name(),
                value: site.value(),
            },
        }
    }
}

impl EntryCapture<'_> {
    pub(crate) fn loadout_xml(
        &self,
        site: &XmlLoadoutPathSite<'_>,
    ) -> Result<AuthoredEntryPath, CatalogError> {
        Ok(
            AuthoredEntryPath::from_text(self.xml_text(&XmlEntrySite::Path(site))?)
                .with_xml_operation(site.observation()),
        )
    }

    pub(super) fn loadout_class_text(
        &self,
        site: &XmlLoadoutClassSite<'_>,
    ) -> Result<AuthoredEntryText, CatalogError> {
        Ok(self
            .xml_text(&XmlEntrySite::Class(site))?
            .with_xml_operation(site.observation()))
    }

    fn xml_text(&self, site: &XmlEntrySite<'_, '_>) -> Result<AuthoredEntryText, CatalogError> {
        let datum = site.datum();
        let operation = datum.operation;
        if operation.read_outcome().is_err() {
            return Err(ExportAccountingError::UnobservedFailure.into());
        }
        let source = operation.source();
        let source = source.entry_origin().map_or_else(
            || {
                if source.archive_input_origin().is_some() {
                    return Err(ExportAccountingError::ConflictingSourceDatum.into());
                }
                Ok(SelectionSource::UnboundLoadout(operation.clone()))
            },
            |origin| {
                self.check_origin(origin)?;
                Ok::<_, CatalogError>(SelectionSource::Archive(origin.clone()))
            },
        )?;
        let (namespace, indices) = datum.attribute.map_or_else(
            || ("xml", vec![u64::from(datum.node)]),
            |ordinal| ("xmlAttribute", vec![u64::from(datum.node), ordinal]),
        );
        let location = SourceLocation::Subresource {
            namespace: namespace.to_owned(),
            indices,
            field: format!("@{}", datum.name),
        };
        let inputs = match site {
            XmlEntrySite::Path(site)
                if site.operation() == LoadoutPathOperation::VehicleModificationName =>
            {
                vec![EntrySelectionUse {
                    source,
                    location,
                    role: SourceUseRole::Selector,
                }]
            }
            XmlEntrySite::Path(_) => vec![
                EntrySelectionUse {
                    source: source.clone(),
                    location: location.clone(),
                    role: SourceUseRole::Payload,
                },
                EntrySelectionUse {
                    source,
                    location,
                    role: SourceUseRole::Conversion,
                },
            ],
            XmlEntrySite::Class(_) => vec![EntrySelectionUse {
                source,
                location,
                role: SourceUseRole::Selector,
            }],
        };
        AuthoredEntryText::captured(self, datum.value, inputs)
    }
}

#[cfg(test)]
#[path = "health_loadout_sdk_tests.rs"]
mod tests;
