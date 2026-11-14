// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_cryxml::{
    SourceAttribute, SourceDocument, SourceElement, SourceReadError, SourceReadOptions,
};
use thiserror::Error;

use crate::soc_entities;
use crate::soc_source_node::{self, SocSourceNode, SocSourceValue};

/// A failure to open the original SOC room registry.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum SocRoomRegistryError {
    /// The original XML is unreadable.
    #[error(transparent)]
    Xml(#[from] SourceReadError),
    /// The parsed XML has another root element.
    #[error("the SOC room registry has no EntityComponentRoom root")]
    Root,
}

/// A room registry with its original XML coordinates and declaration order.
#[derive(Debug)]
pub struct SocRoomRegistry<'input> {
    document: SourceDocument<'input>,
}

impl<'input> SocRoomRegistry<'input> {
    /// Reads a binary or text XML room registry.
    ///
    /// # Errors
    /// Returns an error when the document or registry root is invalid.
    pub fn parse(bytes: &'input [u8]) -> Result<Self, SocRoomRegistryError> {
        let document = SourceDocument::parse(bytes, SourceReadOptions::default())?;
        if !soc_source_node::named(document.root(), "EntityComponentRoom") {
            return Err(SocRoomRegistryError::Root);
        }
        Ok(Self { document })
    }

    /// Borrows the positioned XML including unclassified elements.
    #[must_use]
    pub const fn document(&self) -> &SourceDocument<'_> {
        &self.document
    }

    /// Visits original room declarations without combining repeated identifiers.
    pub fn rooms(&self) -> impl Iterator<Item = SocRoomDeclaration<'_>> {
        SocSourceNode::children(self.document.root(), "Entity")
            .map(|node| SocRoomDeclaration { node })
    }
}

/// One physical room declaration in its owning registry.
#[derive(Debug, Clone, Copy)]
pub struct SocRoomDeclaration<'document> {
    node: SocSourceNode<'document>,
}

impl<'document> SocRoomDeclaration<'document> {
    /// Returns the zero-based slot among the registry root's direct elements.
    #[must_use]
    pub const fn ordinal(self) -> usize {
        self.node.ordinal()
    }

    /// Returns the original declaration and its attribute coordinates.
    #[must_use]
    pub const fn element(self) -> SourceElement<'document> {
        self.node.element()
    }

    /// Returns the original `CryGUID` declarations, including repeated attributes.
    pub fn guid_attributes(self) -> impl Iterator<Item = SourceAttribute<'document>> {
        self.node.attributes("CryGUID")
    }

    /// Reads the original bounds centre vector.
    #[must_use]
    pub fn bounds_center(self) -> SocSourceValue<[f64; 3]> {
        self.vector("BoundsCenter")
    }

    /// Reads the original relative pivot position vector.
    #[must_use]
    pub fn pivot_relative_position(self) -> SocSourceValue<[f64; 3]> {
        self.vector("PivotRelativePos")
    }

    /// Reads the original relative pivot rotation in its stored order.
    #[must_use]
    pub fn pivot_relative_rotation(self) -> SocSourceValue<[f64; 4]> {
        self.vector("PivotRelativeRot")
    }

    fn vector<const N: usize>(self, name: &str) -> SocSourceValue<[f64; N]> {
        self.node
            .read(name, |text| soc_entities::numbers(Some(text)))
    }
}
