// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_cryxml::{XmlFieldKind, XmlWriteField};

use crate::origin_companion::OriginXmlField;
use crate::source_evidence_error::SourceEvidenceError;
use crate::source_scope::SourceScope;
use crate::source_use::{SourceLocation, SourceUse, SourceUseRole};

/// Original contributions to one field written by the binary XML converter.
#[derive(Debug)]
#[non_exhaustive]
pub struct CryXmlFieldUses {
    /// The output field category.
    pub field: OriginXmlField,
    /// The original value and any preceding-name selector actually used.
    pub uses: Vec<SourceUse>,
}

impl SourceScope {
    /// Registers the actual original contributors to an emitted CryXml field.
    ///
    /// # Errors
    /// Returns an error if the original source registry cannot retain evidence.
    pub fn cryxml_field(
        &self,
        field: &XmlWriteField,
    ) -> Result<Option<CryXmlFieldUses>, SourceEvidenceError> {
        let (output, member) = match field.kind() {
            XmlFieldKind::ElementName => (OriginXmlField::ElementName, "name"),
            XmlFieldKind::ClosingElementName => (OriginXmlField::ClosingElementName, "name"),
            XmlFieldKind::AttributeName => (OriginXmlField::AttributeName, "name"),
            XmlFieldKind::AttributeValue => (OriginXmlField::AttributeValue, "value"),
            XmlFieldKind::Text => (OriginXmlField::Text, "text"),
            _ => return Ok(None),
        };
        let node = u64::from(field.node_index());
        let (namespace, indices) = field.attribute_index().map_or_else(
            || ("cryxmlNode", vec![node]),
            |attribute| ("cryxmlAttribute", vec![node, u64::from(attribute)]),
        );
        let mut uses = vec![self.at(
            location(namespace, indices, member),
            SourceUseRole::Conversion,
        )?];
        if let Some(names) = field.preceding_names() {
            uses.push(self.at(
                location(
                    "cryxmlAttributeNames",
                    vec![node, u64::from(names.start), u64::from(names.end)],
                    "suffixSelection",
                ),
                SourceUseRole::Selector,
            )?);
        }
        Ok(Some(CryXmlFieldUses {
            field: output,
            uses,
        }))
    }
}

fn location(namespace: &str, indices: Vec<u64>, field: &str) -> SourceLocation {
    SourceLocation::Subresource {
        namespace: namespace.into(),
        indices,
        field: field.into(),
    }
}
