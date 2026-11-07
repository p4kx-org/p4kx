// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;
use std::io::{self, Write};

use crate::element::Element;
use crate::error::CryXmlError;
use crate::format::{ByteOrder, Signature};
use crate::header::{Header, NODE_LEN, Reader};
use crate::strings::{Span, Strings};

const NO_PARENT: u32 = u32::MAX;
const ATTRIBUTE_LEN: usize = 8;

/// A parsed `CryXml` file: a tree of elements whose strings borrow from the input.
///
/// The tree is built from each node's parent index, which must precede the node, so it is
/// finite and every element appears once. Bytes of the string data that are not valid UTF-8
/// read as `?`.
#[derive(Debug, Clone)]
pub struct Document<'a> {
    signature: Signature,
    byte_order: ByteOrder,
    pub(crate) text: Cow<'a, str>,
    repaired_utf8: bool,
    pub(crate) nodes: Vec<Node>,
    pub(crate) attributes: Vec<(Span, Span)>,
    pub(crate) child_starts: Vec<u32>,
    pub(crate) children: Vec<u32>,
}

#[derive(Debug, Clone)]
pub(crate) struct Node {
    pub(crate) name: Span,
    pub(crate) content: Span,
    pub(crate) first_attribute: u32,
    pub(crate) attribute_count: u16,
    parent: u32,
}

/// Parses `CryXml` of either byte order into a [`Document`].
///
/// # Errors
///
/// Returns [`CryXmlError::NotCryXml`] without a `CryXml` signature, and the other
/// [`CryXmlError`] variants when the input is truncated or corrupt.
pub fn parse(bytes: &[u8]) -> Result<Document<'_>, CryXmlError> {
    let signature = Signature::of(bytes).ok_or(CryXmlError::NotCryXml)?;
    let header = Header::read(bytes, signature)?;
    header.ensure_disjoint()?;
    let strings = Strings::new(header.strings);
    let attributes = read_attributes(header.attributes, header.byte_order, &strings)?;
    let nodes = read_nodes(header.nodes, header.byte_order, &strings, attributes.len())?;
    let (child_starts, children) = link(&nodes);
    Ok(Document {
        signature,
        byte_order: header.byte_order,
        repaired_utf8: strings.repaired_utf8(),
        text: strings.into_text(),
        nodes,
        attributes,
        child_starts,
        children,
    })
}

impl Document<'_> {
    /// Returns whether invalid input string-table bytes were actually repaired.
    #[must_use]
    pub const fn is_lossy(&self) -> bool {
        self.repaired_utf8
    }
    /// Returns the signature the file starts with.
    #[must_use]
    pub const fn signature(&self) -> Signature {
        self.signature
    }

    /// Returns the byte order of the file's integers.
    #[must_use]
    pub const fn byte_order(&self) -> ByteOrder {
        self.byte_order
    }

    /// Returns the number of elements.
    #[must_use]
    pub const fn element_count(&self) -> usize {
        self.nodes.len()
    }

    /// Returns the root element.
    #[must_use]
    pub const fn root(&self) -> Element<'_> {
        Element::new(self, 0)
    }

    /// Writes the document as indented, well-formed UTF-8 XML with LF line endings.
    ///
    /// # Errors
    ///
    /// Returns the first error of `out`.
    pub fn write_xml<W: Write>(&self, out: W) -> io::Result<()> {
        crate::write::write(self, out)
    }
}

fn read_attributes(
    table: &[u8],
    byte_order: ByteOrder,
    strings: &Strings<'_>,
) -> Result<Vec<(Span, Span)>, CryXmlError> {
    let count = table.len() / ATTRIBUTE_LEN;
    let mut reader = Reader::new(table, byte_order);
    let mut attributes = Vec::with_capacity(count);
    for _ in 0..count {
        let name = strings.span(reader.u32()?)?;
        let value = strings.span(reader.u32()?)?;
        attributes.push((name, value));
    }
    Ok(attributes)
}

fn read_nodes(
    table: &[u8],
    byte_order: ByteOrder,
    strings: &Strings<'_>,
    attribute_total: usize,
) -> Result<Vec<Node>, CryXmlError> {
    let count = table.len() / NODE_LEN;
    if count == 0 {
        return Err(CryXmlError::NoRoot);
    }
    let mut reader = Reader::new(table, byte_order);
    let mut nodes = Vec::with_capacity(count);
    for index in (0..count).map(|index| u32::try_from(index).unwrap_or(u32::MAX)) {
        let name = reader.u32()?;
        let content = reader.u32()?;
        let attribute_count = reader.u16()?;
        let _child_count = reader.u16()?;
        let parent = reader.u32()?;
        let first_attribute = reader.u32()?;
        let _first_child = reader.u32()?;
        let _reserved = reader.u32()?;

        let parent_valid = if index == 0 {
            parent == NO_PARENT
        } else {
            parent < index
        };
        if !parent_valid {
            return Err(CryXmlError::BadParent {
                node: index,
                parent,
            });
        }
        let attributes_end = usize::try_from(first_attribute)
            .ok()
            .and_then(|first| first.checked_add(usize::from(attribute_count)));
        if attributes_end.is_none_or(|end| end > attribute_total) {
            return Err(CryXmlError::BadAttributes {
                node: index,
                first: first_attribute,
                count: attribute_count,
            });
        }
        nodes.push(Node {
            name: strings.span(name)?,
            content: strings.span(content)?,
            first_attribute,
            attribute_count,
            parent,
        });
    }
    Ok(nodes)
}

/// Groups the node indices by parent: the children of node `i` are
/// `children[child_starts[i]..child_starts[i + 1]]`, in node order.
fn link(nodes: &[Node]) -> (Vec<u32>, Vec<u32>) {
    let mut child_starts = vec![0_u32; nodes.len() + 1];
    for node in nodes.iter().skip(1) {
        if let Some(count) = child_starts.get_mut(node.parent as usize + 1) {
            *count += 1;
        }
    }
    for index in 1..child_starts.len() {
        let previous = child_starts.get(index - 1).copied().unwrap_or(0);
        if let Some(start) = child_starts.get_mut(index) {
            *start += previous;
        }
    }
    let mut cursor = child_starts.clone();
    let mut children = vec![0_u32; nodes.len().saturating_sub(1)];
    for (index, node) in nodes.iter().enumerate().skip(1) {
        if let Some(next) = cursor.get_mut(node.parent as usize)
            && let Some(slot) = children.get_mut(*next as usize)
        {
            *slot = u32::try_from(index).unwrap_or(u32::MAX);
            *next += 1;
        }
    }
    (child_starts, children)
}
