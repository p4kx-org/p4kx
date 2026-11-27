// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceUseRole, SourceUses, SourceValue, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::{HcdPayload, PRESET, ROOT, parse_guid};
use crate::catalog_read::original::{EntryCapture, EntryXml};
use crate::export_diagnostic::ExportDiagnosticCode;
use crate::guid_text::CatalogGuid;
use crate::schema_fields::every_field_required;
use crate::starmap_xml::{Node, Root, attribute};
use crate::{CatalogError, ExportAccountingError};

/// The original header of a hierarchical context member.
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct HcdDocument {
    version: SourcedValue<String>,
    /// The authored key for joining the physical context-preset definition by GUID.
    preset_guid: SourcedValue<CatalogGuid>,
    preset_text: SourcedValue<String>,
    #[schemars(extend("x-gameText" = true))]
    preset_record_name: SourcedValue<String>,
    attributes: Vec<HcdAttribute>,
}

/// An additional authored attribute of a hierarchical context header.
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[schemars(transform = every_field_required)]
pub struct HcdAttribute {
    name: SourcedValue<String>,
    #[schemars(extend("x-gameText" = true))]
    value: SourcedValue<String>,
}

impl HcdAttribute {
    /// Returns the authored attribute name.
    #[must_use]
    pub const fn name(&self) -> &SourcedValue<String> {
        &self.name
    }

    /// Returns the authored attribute value.
    #[must_use]
    pub const fn value(&self) -> &SourcedValue<String> {
        &self.value
    }
}

impl HcdDocument {
    /// Returns the authored document version.
    #[must_use]
    pub const fn version(&self) -> &SourcedValue<String> {
        &self.version
    }

    /// Returns the parsed context preset GUID.
    #[must_use]
    pub const fn preset_guid(&self) -> &SourcedValue<CatalogGuid> {
        &self.preset_guid
    }

    /// Returns the original preset text.
    #[must_use]
    pub const fn preset_text(&self) -> &SourcedValue<String> {
        &self.preset_text
    }

    /// Returns the authored preset Record name.
    #[must_use]
    pub const fn preset_record_name(&self) -> &SourcedValue<String> {
        &self.preset_record_name
    }

    /// Returns additional attributes in original order.
    #[must_use]
    pub const fn attributes(&self) -> &[HcdAttribute] {
        self.attributes.as_slice()
    }

    pub(super) fn read(
        xml: &EntryXml<'_, '_, '_>,
        capture: &EntryCapture<'_>,
    ) -> Result<(HcdPayload, Option<ExportDiagnosticCode>), CatalogError> {
        let node = xml.root();
        let root = xml.node_source(node)?.selected()?;
        if !node.name().eq_ignore_ascii_case(ROOT) {
            return Ok((
                HcdPayload::Invalid { root: Some(root) },
                Some(ExportDiagnosticCode::UnsupportedValue),
            ));
        }
        let fields = HcdFields {
            xml,
            node,
            sources: capture.source_uses(),
        };
        let preset = attribute(node, PRESET);
        let source = xml.attribute_source(node, PRESET)?;
        let parsed = preset.map_or(SourceValue::Absent, |text| {
            parse_guid(text).map_or(SourceValue::Invalid, |guid| {
                SourceValue::Present(CatalogGuid(guid))
            })
        });
        let complete = !xml.is_lossy();
        let diagnostic = match (&parsed, complete) {
            (_, false) => Some(ExportDiagnosticCode::UnreadableSource),
            (SourceValue::Invalid, true) => Some(ExportDiagnosticCode::MalformedSource),
            _ => None,
        };
        Ok((
            HcdPayload::Readable {
                document: Box::new(Self {
                    version: fields.text("__version")?,
                    preset_guid: fields.sources.value(
                        parsed,
                        &[source.observe(SourceUseRole::Conversion)?],
                        complete,
                    ),
                    preset_text: fields.text(PRESET)?,
                    preset_record_name: fields.text("presetRecordName")?,
                    attributes: fields.extra_attributes(capture)?,
                }),
                root,
                complete,
            },
            diagnostic,
        ))
    }
}

struct HcdFields<'xml, 'read, 'capture, 'input, 'node> {
    xml: &'xml EntryXml<'read, 'capture, 'input>,
    node: Root<'node>,
    sources: &'xml SourceUses,
}

impl HcdFields<'_, '_, '_, '_, '_> {
    fn text(&self, name: &'static str) -> Result<SourcedValue<String>, CatalogError> {
        let state = attribute(self.node, name).map_or(SourceValue::Absent, |value| {
            SourceValue::Present(value.to_owned())
        });
        let source = self.xml.attribute_source(self.node, name)?;
        Ok(self.sources.value(
            state,
            &[source.observe(SourceUseRole::Payload)?],
            !self.xml.is_lossy(),
        ))
    }

    fn extra_attributes(
        &self,
        capture: &EntryCapture<'_>,
    ) -> Result<Vec<HcdAttribute>, CatalogError> {
        self.node
            .attributes()
            .enumerate()
            .filter(|(_, (name, _))| {
                !["__version", PRESET, "presetRecordName"]
                    .iter()
                    .any(|known| name.eq_ignore_ascii_case(known))
            })
            .map(|(ordinal, (name, _))| {
                let original = self
                    .xml
                    .attribute_at(self.node, ordinal)?
                    .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
                let (name_uses, name_complete) = original.selector_inputs()?;
                let datum = original.retain_in(capture)?;
                let (value_uses, value_complete) = datum.contributions();
                Ok(HcdAttribute {
                    name: self.sources.value(
                        SourceValue::Present(name.to_owned()),
                        &name_uses,
                        name_complete && !self.xml.is_lossy(),
                    ),
                    value: self.sources.value(
                        SourceValue::Present(original.as_str().to_owned()),
                        value_uses,
                        value_complete && !self.xml.is_lossy(),
                    ),
                })
            })
            .collect()
    }
}
