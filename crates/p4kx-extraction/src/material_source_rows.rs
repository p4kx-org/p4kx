// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;

use p4kx_model::RawElement;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::material_source_mapping::{attribute_info, content_info, parameter};
use crate::material_xml_info::{
    MaterialAttributeInfo, MaterialContentInfo, MaterialElementInfo, MaterialElementSyntaxInfo,
    MaterialParameterInfo,
};

#[path = "material_source_rows_read.rs"]
mod read;

#[cfg(test)]
#[path = "material_source_rows_tests.rs"]
mod tests;

pub use self::read::MaterialElementRowsError;

/// One tree row and the retained element which owns it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct MaterialElementPart<'source> {
    /// The deterministic preorder number of the owning element.
    pub element: u64,
    /// The original value and its local structure.
    pub row: MaterialElementRow<'source>,
}

impl<'source> MaterialElementPart<'source> {
    /// Pairs a row with its owning element's document-local ID.
    #[must_use]
    pub const fn new(element: u64, row: MaterialElementRow<'source>) -> Self {
        Self { element, row }
    }
}

/// One ordered part of an authored material XML tree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
#[non_exhaustive]
pub enum MaterialElementRow<'source> {
    /// Element header without its lists or children.
    Element {
        /// Owning parent and retained position, absent at the root.
        parent: Option<MaterialElementParent>,
        /// Existing reader name.
        name: Cow<'source, str>,
        /// Original position in the parent, absent at an independently retained root.
        ordinal: Option<usize>,
        /// Original qualified name.
        qualified_name: Cow<'source, str>,
        /// Proved expanded namespace URI, when available.
        namespace: Option<Cow<'source, str>>,
        /// Independently ordered binary node text.
        binary_text: Option<Cow<'source, str>>,
        /// Number of retained decoded attributes.
        attributes: usize,
        /// Number of retained qualified declarations.
        declarations: usize,
        /// Number of retained mixed content items.
        contents: usize,
        /// Number of retained own children.
        children: usize,
    },
    /// One decoded attribute in reader order.
    DecodedAttribute {
        /// Stable zero-based position among decoded attributes.
        position: usize,
        /// Decoded name and value.
        value: MaterialParameterInfo<'source>,
    },
    /// One authored attribute or namespace declaration.
    Declaration {
        /// Stable zero-based position among authored declarations.
        position: usize,
        /// Original qualified name, tokens and known namespace semantics.
        value: MaterialAttributeInfo<'source>,
    },
    /// One authored content item.
    Content {
        /// Stable zero-based position in the retained content list.
        position: usize,
        /// Original text, element position or independent syntax item.
        value: MaterialContentInfo<'source>,
    },
}

/// An authored element's placement among the retained children of its parent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct MaterialElementParent {
    /// The earlier preorder ID of the owning parent.
    pub element: u64,
    /// The zero-based retained child position.
    pub position: usize,
}

/// An authored element cannot be visited or one of the individual rows was refused.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum MaterialElementWriteError<E> {
    /// The caller interrupted the traversal or refused a projected row.
    #[error("material element output was interrupted")]
    Interrupted(E),
    /// The number of elements exceeds the representable ID space.
    #[error("material element count exceeds the wire width")]
    ElementCountOverflow,
}

impl<'source> MaterialElementRow<'source> {
    /// Visits every retained row in preorder with the borrowed original ordinal path.
    /// The root path is empty. A missing original coordinate propagates to its descendants.
    ///
    /// # Errors
    /// Returns the original observer error or an unrepresentable element count.
    pub fn visit<E>(
        root: &'source RawElement,
        mut observe: impl FnMut(Option<&[usize]>, MaterialElementPart<'source>) -> Result<(), E>,
    ) -> Result<(), MaterialElementWriteError<E>> {
        let mut position = OriginalPath::default();
        Self::visit_element(root, None, &mut 0, &mut position, &mut observe)
    }

    /// Reassembles a complete tree from its ordered headers and projected subrows.
    ///
    /// # Errors
    /// Returns a structural error if any row is absent, duplicated or misplaced.
    pub fn reconstruct(
        rows: impl IntoIterator<Item = MaterialElementPart<'source>>,
    ) -> Result<MaterialElementInfo<'source>, MaterialElementRowsError> {
        read::reconstruct(rows)
    }

    fn visit_element<E>(
        element: &'source RawElement,
        parent: Option<MaterialElementParent>,
        ordinal: &mut u64,
        original: &mut OriginalPath,
        observe: &mut impl FnMut(Option<&[usize]>, MaterialElementPart<'source>) -> Result<(), E>,
    ) -> Result<(), MaterialElementWriteError<E>> {
        let identity = *ordinal;
        *ordinal = ordinal
            .checked_add(1)
            .ok_or(MaterialElementWriteError::ElementCountOverflow)?;
        Self::emit_element(element, parent, |row| {
            observe(original.get(), MaterialElementPart::new(identity, row))
        })
        .map_err(MaterialElementWriteError::Interrupted)?;
        for (position, child) in element.children.iter().enumerate() {
            let restore = original.enter(child.authored.ordinal);
            Self::visit_element(
                child,
                Some(MaterialElementParent {
                    element: identity,
                    position,
                }),
                ordinal,
                original,
                observe,
            )?;
            original.leave(restore);
        }
        Ok(())
    }

    fn emit_element<E>(
        element: &'source RawElement,
        parent: Option<MaterialElementParent>,
        mut observe: impl FnMut(Self) -> Result<(), E>,
    ) -> Result<(), E> {
        observe(Self::header(element, parent))?;
        for (position, value) in element.attributes.iter().enumerate() {
            observe(Self::DecodedAttribute {
                position,
                value: parameter(value),
            })?;
        }
        for (position, value) in element.authored.attributes.iter().enumerate() {
            observe(Self::Declaration {
                position,
                value: attribute_info(value),
            })?;
        }
        for (position, value) in element.authored.content.iter().enumerate() {
            observe(Self::Content {
                position,
                value: content_info(value),
            })?;
        }
        Ok(())
    }

    fn header(element: &'source RawElement, parent: Option<MaterialElementParent>) -> Self {
        let syntax = &element.authored;
        Self::Element {
            parent,
            name: Cow::Borrowed(&element.name),
            ordinal: syntax.ordinal,
            qualified_name: Cow::Borrowed(&syntax.qualified_name),
            namespace: syntax.namespace.as_deref().map(Cow::Borrowed),
            binary_text: syntax.binary_text.as_deref().map(Cow::Borrowed),
            attributes: element.attributes.len(),
            declarations: syntax.attributes.len(),
            contents: syntax.content.len(),
            children: element.children.len(),
        }
    }
}

#[derive(Default)]
struct OriginalPath {
    indices: Vec<usize>,
    incomplete: bool,
}

#[derive(Clone, Copy)]
enum Leave {
    Known,
    Missing,
    Incomplete,
}

impl OriginalPath {
    fn get(&self) -> Option<&[usize]> {
        (!self.incomplete).then_some(&self.indices)
    }

    fn enter(&mut self, ordinal: Option<usize>) -> Leave {
        if self.incomplete {
            return Leave::Incomplete;
        }
        if let Some(ordinal) = ordinal {
            self.indices.push(ordinal);
            Leave::Known
        } else {
            self.incomplete = true;
            Leave::Missing
        }
    }

    fn leave(&mut self, restore: Leave) {
        match restore {
            Leave::Known => {
                let _ = self.indices.pop();
            }
            Leave::Missing => self.incomplete = false,
            Leave::Incomplete => {}
        }
    }
}

pub(super) struct ElementHeader<'source> {
    pub(super) parent: Option<MaterialElementParent>,
    pub(super) element: MaterialElementInfo<'source>,
    pub(super) attributes: usize,
    pub(super) declarations: usize,
    pub(super) contents: usize,
    pub(super) children: usize,
}

impl<'source> ElementHeader<'source> {
    pub(super) fn read(row: MaterialElementRow<'source>) -> Result<Self, MaterialElementRowsError> {
        let MaterialElementRow::Element {
            parent,
            name,
            ordinal,
            qualified_name,
            namespace,
            binary_text,
            attributes,
            declarations,
            contents,
            children,
        } = row
        else {
            return Err(MaterialElementRowsError::Structure);
        };
        Ok(Self {
            parent,
            element: MaterialElementInfo {
                name,
                attributes: Vec::new(),
                children: Vec::new(),
                syntax: MaterialElementSyntaxInfo {
                    ordinal,
                    qualified_name,
                    namespace,
                    attributes: Vec::new(),
                    content: Vec::new(),
                    binary_text,
                },
            },
            attributes,
            declarations,
            contents,
            children,
        })
    }
}
