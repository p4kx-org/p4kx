// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use roxmltree::Attributes as TextAttributes;

use crate::element::Attributes;
use crate::source_position::{self, SourceAttributePosition};

/// A borrowed parsed attribute and its original source coordinate.
#[derive(Debug, Clone)]
pub struct SourceAttribute<'document> {
    name: &'document str,
    namespace: Option<&'document str>,
    value: &'document str,
    position: SourceAttributePosition,
}

impl<'document> SourceAttribute<'document> {
    /// Returns the text attribute's local name or the stored CryXml name.
    #[must_use]
    pub const fn name(&self) -> &'document str {
        self.name
    }

    /// Returns the text attribute's expanded namespace URI when present.
    #[must_use]
    pub const fn namespace(&self) -> Option<&'document str> {
        self.namespace
    }

    /// Returns the parsed attribute value.
    #[must_use]
    pub const fn value(&self) -> &'document str {
        self.value
    }

    /// Returns the original coordinate, independent of text normalization.
    #[must_use]
    pub const fn position(&self) -> &SourceAttributePosition {
        &self.position
    }

    pub(crate) const fn into_position(self) -> SourceAttributePosition {
        self.position
    }
}

pub(crate) enum SourceAttributes<'document> {
    Text {
        attributes: TextAttributes<'document, 'document>,
        prefix_bytes: usize,
    },
    CryXml {
        attributes: Attributes<'document>,
        index: u32,
        node_index: u32,
    },
}

impl<'document> Iterator for SourceAttributes<'document> {
    type Item = SourceAttribute<'document>;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Text {
                attributes,
                prefix_bytes,
            } => {
                let attribute = attributes.next()?;
                Some(SourceAttribute {
                    name: attribute.name(),
                    namespace: attribute.namespace(),
                    value: attribute.value(),
                    position: SourceAttributePosition::Text(source_position::original_range(
                        attribute.range(),
                        *prefix_bytes,
                    )),
                })
            }
            Self::CryXml {
                attributes,
                index,
                node_index,
            } => {
                let attribute = attributes.next()?;
                let attribute_index = *index;
                *index += 1;
                Some(SourceAttribute {
                    name: attribute.name(),
                    namespace: None,
                    value: attribute.value(),
                    position: SourceAttributePosition::CryXml {
                        node_index: *node_index,
                        attribute_index,
                    },
                })
            }
        }
    }
}
