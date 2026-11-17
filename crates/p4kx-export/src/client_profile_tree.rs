// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::cryxml::{
    SourceContent, SourceContentPosition, SourceDeclaration, SourceElement, SourceSpanError,
    SourceSyntax, SourceText,
};
use p4kx_extraction::{CancelToken, SourceLocation, SourceUseRole, SourceValue, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::check_cancel;
use crate::catalog_read::original::EntrySourceXml;
use crate::schema_fields::every_field_required;
use crate::{CatalogError, ExportAccountingError};

/// An authored XML element with its position, declarations and direct text content.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct ProfileElement {
    parent: SourcedValue<Option<u32>>,
    position: SourcedValue<SourceLocation>,
    name: SourcedValue<String>,
    declarations: Vec<ProfileDeclaration>,
    content: Vec<ProfileText>,
}

impl ProfileElement {
    /// Borrows the parent element index, with an explicit null for the root.
    #[must_use]
    pub const fn parent(&self) -> &SourcedValue<Option<u32>> {
        &self.parent
    }

    /// Borrows the physical XML extent or CryXml element index.
    #[must_use]
    pub const fn position(&self) -> &SourcedValue<SourceLocation> {
        &self.position
    }

    /// Borrows the authored qualified element name.
    #[must_use]
    pub const fn name(&self) -> &SourcedValue<String> {
        &self.name
    }

    /// Lists attributes and namespace declarations in original order.
    #[must_use]
    pub fn declarations(&self) -> &[ProfileDeclaration] {
        &self.declarations
    }

    /// Lists the element's direct text values in original order.
    #[must_use]
    pub fn content(&self) -> &[ProfileText] {
        &self.content
    }
}

/// An original attribute or namespace declaration.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct ProfileDeclaration {
    name: SourcedValue<String>,

    value: ProfileText,
}

impl ProfileDeclaration {
    /// Borrows the authored qualified declaration name.
    #[must_use]
    pub const fn name(&self) -> &SourcedValue<String> {
        &self.name
    }

    /// Borrows the protected value and any alternate lexical spelling.
    #[must_use]
    pub const fn value(&self) -> &ProfileText {
        &self.value
    }
}

/// An authored text value and any distinct original lexical spelling.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct ProfileText {
    #[schemars(extend("x-gameText" = true))]
    value: SourcedValue<String>,
    #[schemars(extend("x-gameText" = true))]
    // The alternate lexical spelling is needed only when XML decoding changes it.
    lexeme: Option<SourcedValue<String>>,
}

impl ProfileText {
    /// Borrows the protected decoded value with its own origin.
    #[must_use]
    pub const fn value(&self) -> &SourcedValue<String> {
        &self.value
    }

    /// Borrows the protected source spelling when it differs from the decoded value.
    #[must_use]
    pub const fn lexeme(&self) -> Option<&SourcedValue<String>> {
        self.lexeme.as_ref()
    }
}

pub(super) fn read(
    xml: &EntrySourceXml<'_>,
    cancel: Option<&CancelToken>,
) -> Result<Vec<ProfileElement>, CatalogError> {
    let mut pending = vec![(xml.root(), None)];
    let mut elements = Vec::new();
    while let Some((node, parent)) = pending.pop() {
        check_cancel(cancel)?;
        let index =
            u32::try_from(elements.len()).map_err(|_| ExportAccountingError::CounterOverflow)?;
        let parent = parent
            .map(|index: u32| {
                elements
                    .get(index as usize)
                    .map(|element| (index, element))
                    .ok_or(ExportAccountingError::ConflictingSourceDatum)
            })
            .transpose()?;
        elements.push(element(xml, node, parent)?);
        let first_child = pending.len();
        pending.extend(node.elements().map(|child| (child, Some(index))));
        pending
            .get_mut(first_child..)
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?
            .reverse();
    }
    Ok(elements)
}

fn element(
    xml: &EntrySourceXml<'_>,
    node: SourceElement<'_>,
    parent: Option<(u32, &ProfileElement)>,
) -> Result<ProfileElement, CatalogError> {
    let syntax = node.syntax().map_err(span_error)?;
    let position = xml.position(node)?;
    let parent = match parent {
        Some((index, parent)) => xml.sources().value_with_origins(
            SourceValue::Present(Some(index)),
            &[position.origin(), parent.position.origin()],
        )?,
        None => position.map_ref(|_| None),
    };
    let location = SourceLocation::xml_element(&node.position(), "$name")
        .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
    let name = xml.retain(
        location,
        syntax.qualified_name().to_owned(),
        SourceUseRole::Payload,
    )?;
    let declarations = syntax
        .declarations()
        .map(|entry| declaration(xml, &entry.map_err(span_error)?))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ProfileElement {
        parent,
        position,
        name,
        declarations,
        content: content(xml, &syntax)?,
    })
}

fn declaration(
    xml: &EntrySourceXml<'_>,
    declaration: &SourceDeclaration<'_>,
) -> Result<ProfileDeclaration, CatalogError> {
    let name = declaration.qualified_name();
    let location = SourceLocation::xml_attribute(declaration.position(), name)
        .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
    let name = xml.retain(location.clone(), name.to_owned(), SourceUseRole::Selector)?;
    Ok(ProfileDeclaration {
        name,
        value: original_text(xml, location, declaration.value(), declaration.lexeme())?,
    })
}

fn content(
    xml: &EntrySourceXml<'_>,
    syntax: &SourceSyntax<'_>,
) -> Result<Vec<ProfileText>, CatalogError> {
    if let Some(text) = syntax.binary_text() {
        return Ok(vec![node_text(xml, &text)?]);
    }
    syntax
        .content()
        .map_err(span_error)?
        .filter_map(|item| match item {
            Ok(SourceContent::Text(text)) => Some(node_text(xml, &text)),
            Ok(_) => None,
            Err(error) => Some(Err(span_error(error))),
        })
        .collect()
}

fn node_text(xml: &EntrySourceXml<'_>, text: &SourceText<'_>) -> Result<ProfileText, CatalogError> {
    let (namespace, indices) = match text.position() {
        SourceContentPosition::Text(range) => {
            ("textXmlContent", vec![range.start as u64, range.end as u64])
        }
        SourceContentPosition::CryXml(index) => ("cryxmlContent", vec![u64::from(*index)]),
        _ => return Err(ExportAccountingError::ConflictingSourceDatum.into()),
    };
    let location = SourceLocation::Subresource {
        namespace: namespace.to_owned(),
        indices,
        field: "$content".to_owned(),
    };
    original_text(xml, location, text.value(), text.lexeme())
}

fn original_text(
    xml: &EntrySourceXml<'_>,
    location: SourceLocation,
    value: &str,
    lexeme: Option<&str>,
) -> Result<ProfileText, CatalogError> {
    let lexeme = lexeme.filter(|lexeme| *lexeme != value);
    let retained_lexeme = lexeme
        .map(|lexeme| xml.retain(location.clone(), lexeme.to_owned(), SourceUseRole::Payload))
        .transpose()?;
    let role = if retained_lexeme.is_some() {
        SourceUseRole::Conversion
    } else {
        SourceUseRole::Payload
    };
    Ok(ProfileText {
        value: xml.retain(location, value.to_owned(), role)?,
        lexeme: retained_lexeme,
    })
}

fn span_error(_: SourceSpanError) -> CatalogError {
    ExportAccountingError::ConflictingSourceDatum.into()
}
