// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{EntityNode, Target, Unresolved};
use p4kx_extraction::{SourceUseRole, SourceValue, SourcedValue};

use super::Capture;
use crate::CatalogError;
use crate::catalog_read::original::OriginalDatum;
use crate::guid_text::CatalogGuid;
use crate::record_header_field::RecordHeaderField;

pub(crate) struct AttachmentValues {
    pub(crate) guid: SourcedValue<CatalogGuid>,
    pub(crate) class_name: SourcedValue<String>,
}

impl Capture<'_> {
    pub(crate) fn attached_values(
        &self,
        child: &EntityNode,
    ) -> Result<AttachmentValues, CatalogError> {
        let guid = child.guid();
        let sources = &self.name_sources;
        let Some(input) = self.input else {
            return Ok(AttachmentValues {
                guid: sources.value(SourceValue::Present(CatalogGuid(guid)), &[], false),
                class_name: sources.value(
                    SourceValue::Present(child.class_name().to_owned()),
                    &[],
                    false,
                ),
            });
        };
        let target = child
            .attachment_target()
            .map(|selection| self.retained_target(guid, selection))
            .transpose()?
            .flatten();
        let (uses, complete) = target
            .as_ref()
            .map_or((&[][..], false), OriginalDatum::contributions);
        let mut name_uses: Vec<_> = uses
            .iter()
            .filter(|used| used.role() == SourceUseRole::Selector)
            .cloned()
            .collect();
        if let Some(record) = child.record_index() {
            name_uses.push(input.origins().dataforge.at(
                RecordHeaderField::RecordName.location(record),
                SourceUseRole::Conversion,
            )?);
        }
        let class_name = sources.value(
            SourceValue::Present(child.class_name().to_owned()),
            &name_uses,
            complete && child.record_index().is_some(),
        );
        let guid = match target {
            Some(target) => target.map(CatalogGuid)?.into_cell(sources),
            None => sources.value(SourceValue::Present(CatalogGuid(guid)), &[], false),
        };
        Ok(AttachmentValues { guid, class_name })
    }

    pub(crate) fn unresolved_attachment_values(
        &self,
        unresolved: &Unresolved,
    ) -> Result<AttachmentValues, CatalogError> {
        let sources = &self.name_sources;
        let (guid, class_name) = match unresolved.target() {
            Target::Reference(guid) => {
                let value = match self.input.zip(unresolved.reference_selection()) {
                    Some((input, selection)) => {
                        let receipt = self.lock()?.references.get(selection, *guid)?;
                        receipt
                            .retained_for(input)?
                            .map(CatalogGuid)?
                            .into_cell(sources)
                    }
                    None => sources.value(SourceValue::Present(CatalogGuid(*guid)), &[], false),
                };
                (value, sources.value(SourceValue::Absent, &[], false))
            }
            Target::ClassName(name) => {
                let value = match (self.input, unresolved.class_selection()) {
                    (Some(_), Some((selection, usage))) => {
                        self.unresolved_class_name(selection, name, usage)?
                    }
                    _ => sources.value(SourceValue::Present(name.clone()), &[], false),
                };
                (sources.value(SourceValue::Absent, &[], false), value)
            }
            Target::Unreadable => (
                sources.value(SourceValue::Unreadable, &[], false),
                sources.value(SourceValue::Unreadable, &[], false),
            ),
        };
        Ok(AttachmentValues { guid, class_name })
    }
}
