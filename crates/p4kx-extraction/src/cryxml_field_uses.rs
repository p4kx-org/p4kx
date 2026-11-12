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
        self.cryxml_field_in(field, None)
    }

    /// Registers the actual contributors to a CryXml field in an embedded document.
    ///
    /// `decoded_offset` is the document start in the decoded physical file.
    ///
    /// # Errors
    /// Returns an error if the original source registry cannot retain evidence.
    pub fn cryxml_field_at(
        &self,
        field: &XmlWriteField,
        decoded_offset: u64,
    ) -> Result<Option<CryXmlFieldUses>, SourceEvidenceError> {
        self.cryxml_field_in(field, Some(decoded_offset))
    }

    fn cryxml_field_in(
        &self,
        field: &XmlWriteField,
        base: Option<u64>,
    ) -> Result<Option<CryXmlFieldUses>, SourceEvidenceError> {
        let (output, member) = match field.kind() {
            XmlFieldKind::ElementName => (OriginXmlField::ElementName, "name"),
            XmlFieldKind::ClosingElementName => (OriginXmlField::ClosingElementName, "name"),
            XmlFieldKind::AttributeName => (OriginXmlField::AttributeName, "name"),
            XmlFieldKind::AttributeValue => (OriginXmlField::AttributeValue, "value"),
            XmlFieldKind::Text => (OriginXmlField::Text, "text"),
            _ => return Ok(None),
        };
        let (node_namespace, attribute_namespace, names_namespace) = if base.is_some() {
            (
                "embeddedCryXmlNode",
                "embeddedCryXmlAttribute",
                "embeddedCryXmlAttributeNames",
            )
        } else {
            ("cryxmlNode", "cryxmlAttribute", "cryxmlAttributeNames")
        };
        let node = u64::from(field.node_index());
        let primary = field.attribute_index().map_or_else(
            || location(node_namespace, &[node], member, base),
            |attribute| {
                location(
                    attribute_namespace,
                    &[node, u64::from(attribute)],
                    member,
                    base,
                )
            },
        );
        let mut uses = vec![self.at(primary, SourceUseRole::Conversion)?];
        if let Some(names) = field.preceding_names() {
            uses.push(self.at(
                location(
                    names_namespace,
                    &[node, u64::from(names.start), u64::from(names.end)],
                    "suffixSelection",
                    base,
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

fn location(namespace: &str, indices: &[u64], field: &str, base: Option<u64>) -> SourceLocation {
    SourceLocation::Subresource {
        namespace: namespace.into(),
        indices: base.into_iter().chain(indices.iter().copied()).collect(),
        field: field.into(),
    }
}
