// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_model::{
    AuthoredAttributeKind, AuthoredContent, MaterialEncoding, MaterialSource,
    MaterialSourceLossKind, RawElement,
};
use thiserror::Error;

use crate::field_origin::SharedFieldOrigin;
use crate::source_details::SourceCoordinateOmission;
use crate::{
    FieldOrigin, FieldOrigins, FieldOriginsError, FieldPath, FieldPathError, SourceEvidenceError,
    SourceLocation, SourceScope, SourceUse, SourceUseRole, SourceUses,
};

/// Failure to project the retained original positions of authored material values.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum MaterialSourceOriginError {
    /// The caller stopped the traversal.
    #[error("material source projection was cancelled")]
    Cancelled,
    /// A registered physical source use was unavailable.
    #[error(transparent)]
    Evidence(#[from] SourceEvidenceError),
    /// A JSON address could not be represented.
    #[error(transparent)]
    Path(#[from] FieldPathError),
    /// Another projection already owned this address.
    #[error(transparent)]
    Duplicate(#[from] FieldOriginsError),
}

/// Original evidence and selectors shared by a retained material library.
pub struct MaterialSourceEvidence<'a> {
    uses: &'a SourceUses,
    scope: Option<&'a SourceScope>,
    shared: &'a SharedFieldOrigin,
}

impl<'a> MaterialSourceEvidence<'a> {
    /// Binds the physical payload scope and any original selectors to one evidence inventory.
    #[must_use]
    pub const fn new(
        uses: &'a SourceUses,
        scope: Option<&'a SourceScope>,
        shared: &'a SharedFieldOrigin,
    ) -> Self {
        Self {
            uses,
            scope,
            shared,
        }
    }

    /// Projects original coordinates into the wire shape of a canonical material source.
    ///
    /// # Errors
    /// Returns an error when an original contributor or output address cannot be registered.
    pub fn project(
        &self,
        source: &MaterialSource,
        address: &FieldPath,
        fields: &mut FieldOrigins,
    ) -> Result<(), MaterialSourceOriginError> {
        self.project_with(source, address, fields, &|| false)
    }

    /// Projects authored field origins while checking for cancellation at source coordinates.
    ///
    /// # Errors
    /// Returns cancellation or an error registering a contributor or output address.
    pub fn project_with(
        &self,
        source: &MaterialSource,
        address: &FieldPath,
        fields: &mut FieldOrigins,
        cancelled: &impl Fn() -> bool,
    ) -> Result<(), MaterialSourceOriginError> {
        let mut writer = Writer {
            evidence: self,
            fields,
            positions: Vec::new(),
            encoding: source.encoding,
            cancelled,
        };
        writer.check()?;
        writer.header(address, source)?;
        if let Some(root) = &source.root {
            writer.element(&member(address, "root")?, root)?;
        }
        Ok(())
    }

    fn origin(
        &self,
        namespace: &str,
        indices: &[u64],
        field: &str,
        role: SourceUseRole,
    ) -> Result<FieldOrigin, MaterialSourceOriginError> {
        let used = self
            .scope
            .map(|scope| {
                scope.at(
                    SourceLocation::Subresource {
                        namespace: namespace.into(),
                        indices: indices.to_vec(),
                        field: field.into(),
                    },
                    role,
                )
            })
            .transpose()?;
        self.combine(used.as_slice())
    }

    fn opaque(&self, role: SourceUseRole) -> Result<FieldOrigin, MaterialSourceOriginError> {
        let used = self
            .scope
            .map(|scope| scope.at_opaque(role, SourceCoordinateOmission::Unavailable))
            .transpose()?;
        self.combine(used.as_slice())
    }

    fn combine(&self, local: &[SourceUse]) -> Result<FieldOrigin, MaterialSourceOriginError> {
        Ok(self
            .uses
            .origin_with_shared(self.shared, local, self.scope.is_some())?)
    }
}

struct Writer<'a, 'output> {
    evidence: &'a MaterialSourceEvidence<'a>,
    fields: &'output mut FieldOrigins,
    positions: Vec<u64>,
    encoding: MaterialEncoding,
    cancelled: &'output dyn Fn() -> bool,
}

impl Writer<'_, '_> {
    fn check(&self) -> Result<(), MaterialSourceOriginError> {
        if (self.cancelled)() {
            Err(MaterialSourceOriginError::Cancelled)
        } else {
            Ok(())
        }
    }

    fn header(
        &mut self,
        path: &FieldPath,
        source: &MaterialSource,
    ) -> Result<(), MaterialSourceOriginError> {
        let header = self.evidence.origin(
            "materialXmlDocument",
            &[],
            "encoding",
            SourceUseRole::Conversion,
        )?;
        self.fields.insert(member(path, "encoding")?, header)?;
        let boundaries = member(path, "losses")?;
        for (ordinal, loss) in source.losses.iter().enumerate() {
            self.check()?;
            let logical: Vec<_> = loss.indices.iter().map(|&index| index as u64).collect();
            let origin = if loss.kind == MaterialSourceLossKind::RepairedEncoding {
                self.evidence.opaque(SourceUseRole::Conversion)?
            } else {
                self.evidence.origin(
                    "materialXmlElement",
                    &logical,
                    "captureBoundary",
                    SourceUseRole::Conversion,
                )?
            };
            let item = index(&boundaries, ordinal)?;
            self.members(&item, &["kind", "indices"], &origin)?;
            let positions = member(&item, "indices")?;
            for (position, _) in logical.iter().enumerate() {
                self.check()?;
                self.fields
                    .insert(index(&positions, position)?, origin.clone())?;
            }
        }
        self.entries(path, source)
    }

    fn entries(
        &mut self,
        path: &FieldPath,
        source: &MaterialSource,
    ) -> Result<(), MaterialSourceOriginError> {
        let list = source.submaterial_list_ordinal;
        for (position, row) in source.entries.iter().enumerate() {
            self.check()?;
            let origin = if let Some(list) = list {
                self.evidence.origin(
                    "materialXmlElement",
                    &[list as u64, row.ordinal as u64],
                    "elementKind",
                    SourceUseRole::Selector,
                )?
            } else {
                self.evidence.opaque(SourceUseRole::Selector)?
            };
            self.members(
                &index(&member(path, "entries")?, position)?,
                &["ordinal", "element_name", "disposition"],
                &origin,
            )?;
        }
        Ok(())
    }

    fn element(
        &mut self,
        path: &FieldPath,
        element: &RawElement,
    ) -> Result<(), MaterialSourceOriginError> {
        let syntax = member(path, "syntax")?;
        let name = self.site(
            "materialXmlElement",
            "qualifiedName",
            SourceUseRole::Payload,
        )?;
        self.fields.insert(member(path, "name")?, name.clone())?;
        self.fields
            .insert(member(&syntax, "qualified_name")?, name)?;
        let ordinal = self.site(
            "materialXmlElement",
            "childOrdinal",
            SourceUseRole::Selector,
        )?;
        self.fields.insert(member(&syntax, "ordinal")?, ordinal)?;
        let namespace = if element.authored.namespace.is_some() {
            self.evidence.opaque(SourceUseRole::Conversion)?
        } else {
            self.site("materialXmlElement", "namespace", SourceUseRole::Selector)?
        };
        self.fields
            .insert(member(&syntax, "namespace")?, namespace)?;
        let binary_text = self.site("materialXmlElement", "binaryText", SourceUseRole::Payload)?;
        self.fields
            .insert(member(&syntax, "binary_text")?, binary_text)?;
        self.attributes(path, &syntax, element)?;
        self.content(&syntax, &element.authored.content)?;
        let children = member(path, "children")?;
        for (position, child) in element.children.iter().enumerate() {
            self.check()?;
            if let Some(ordinal) = child.authored.ordinal {
                self.positions.push(ordinal as u64);
                self.element(&index(&children, position)?, child)?;
                self.positions.pop();
            }
        }
        Ok(())
    }

    fn attributes(
        &mut self,
        path: &FieldPath,
        syntax: &FieldPath,
        element: &RawElement,
    ) -> Result<(), MaterialSourceOriginError> {
        let parsed = member(path, "attributes")?;
        for (position, _) in element.attributes.iter().enumerate() {
            self.check()?;
            self.positions.push(position as u64);
            let origin = self.site("materialXmlAttribute", "attribute", SourceUseRole::Payload)?;
            self.positions.pop();
            self.members(&index(&parsed, position)?, &["name", "value"], &origin)?;
        }
        let declarations = member(syntax, "attributes")?;
        for (position, attribute) in element.authored.attributes.iter().enumerate() {
            self.check()?;
            self.positions.push(position as u64);
            let origin = self.site(
                "materialXmlDeclaration",
                "declaration",
                SourceUseRole::Payload,
            )?;
            self.positions.pop();
            let attribute_path = index(&declarations, position)?;
            self.members(&attribute_path, &["name", "value", "lexeme"], &origin)?;
            let kind = member(&attribute_path, "kind")?;
            self.fields.insert(member(&kind, "type")?, origin.clone())?;
            match &attribute.kind {
                AuthoredAttributeKind::Namespace { .. } => {
                    self.fields.insert(member(&kind, "prefix")?, origin)?;
                }
                AuthoredAttributeKind::Attribute { namespace } => {
                    let namespace_origin = if namespace.is_some() {
                        self.evidence.opaque(SourceUseRole::Conversion)?
                    } else {
                        origin
                    };
                    self.fields
                        .insert(member(&kind, "namespace")?, namespace_origin)?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn content(
        &mut self,
        path: &FieldPath,
        content: &[AuthoredContent],
    ) -> Result<(), MaterialSourceOriginError> {
        let list = member(path, "content")?;
        for (position, item) in content.iter().enumerate() {
            self.check()?;
            let location = index(&list, position)?;
            let (ordinal, namespace, fields): (_, _, &[_]) = match item {
                AuthoredContent::Element { ordinal }
                    if self.encoding == MaterialEncoding::CryXml =>
                {
                    (*ordinal, "materialXmlElement", &["type", "ordinal"])
                }
                AuthoredContent::Element { .. } => {
                    (position, "materialXmlContent", &["type", "ordinal"])
                }
                AuthoredContent::Text { .. } => {
                    (position, "materialXmlContent", &["type", "value", "lexeme"])
                }
                AuthoredContent::Comment { .. } => {
                    (position, "materialXmlContent", &["type", "value"])
                }
                AuthoredContent::ProcessingInstruction { .. } => {
                    (position, "materialXmlContent", &["type", "target", "value"])
                }
                _ => continue,
            };
            self.positions.push(ordinal as u64);
            let origin = self.site(namespace, "content", SourceUseRole::Payload)?;
            self.positions.pop();
            self.members(&location, fields, &origin)?;
        }
        Ok(())
    }

    fn site(
        &self,
        namespace: &str,
        field: &str,
        role: SourceUseRole,
    ) -> Result<FieldOrigin, MaterialSourceOriginError> {
        self.evidence
            .origin(namespace, &self.positions, field, role)
    }

    fn members(
        &mut self,
        path: &FieldPath,
        names: &[&str],
        origin: &FieldOrigin,
    ) -> Result<(), MaterialSourceOriginError> {
        for name in names {
            self.fields.insert(member(path, name)?, origin.clone())?;
        }
        Ok(())
    }
}

fn member(path: &FieldPath, field: &str) -> Result<FieldPath, FieldPathError> {
    let mut address = path.clone();
    address.push_field(field)?;
    Ok(address)
}

fn index(path: &FieldPath, position: usize) -> Result<FieldPath, FieldPathError> {
    let mut address = path.clone();
    address.push_index(position)?;
    Ok(address)
}
