// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use thiserror::Error;

use crate::material_xml_info::MaterialElementInfo;

use super::{ElementHeader, MaterialElementParent, MaterialElementPart, MaterialElementRow};

/// A streamed material element tree cannot be rebuilt exactly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum MaterialElementRowsError {
    /// A row was absent, out of order, had an invalid position or had an invalid owner.
    #[error("material element rows do not match their declared positions and counts")]
    Structure,
    /// Checked element IDs exceed the wire width.
    #[error("material element count exceeds the wire width")]
    ElementCountOverflow,
}

/// Reconstructs a tree while retaining only open ancestors and their observed contents.
pub(super) fn reconstruct<'source>(
    rows: impl IntoIterator<Item = MaterialElementPart<'source>>,
) -> Result<MaterialElementInfo<'source>, MaterialElementRowsError> {
    let mut stack: Vec<Pending<'source>> = Vec::new();
    let mut root = None;
    let mut next = 0_u64;
    for part in rows {
        while stack.last().is_some_and(Pending::complete) {
            attach(&mut stack, &mut root)?;
        }
        if root.is_some() {
            return Err(MaterialElementRowsError::Structure);
        }
        match part.row {
            row @ MaterialElementRow::Element { .. } => {
                let header = ElementHeader::read(row)?;
                let parent = stack.last().map(Pending::next_child);
                if part.element != next
                    || header.parent != parent
                    || stack
                        .last()
                        .is_some_and(|entry| !entry.attributes_complete())
                {
                    return Err(MaterialElementRowsError::Structure);
                }
                next = next
                    .checked_add(1)
                    .ok_or(MaterialElementRowsError::ElementCountOverflow)?;
                stack.push(Pending {
                    id: part.element,
                    header,
                });
            }
            row => stack
                .last_mut()
                .filter(|entry| entry.id == part.element)
                .ok_or(MaterialElementRowsError::Structure)?
                .append(row)?,
        }
    }
    while stack.last().is_some_and(Pending::complete) {
        attach(&mut stack, &mut root)?;
    }
    root.ok_or(MaterialElementRowsError::Structure)
}

fn attach<'source>(
    stack: &mut Vec<Pending<'source>>,
    root: &mut Option<MaterialElementInfo<'source>>,
) -> Result<(), MaterialElementRowsError> {
    let completed = stack.pop().ok_or(MaterialElementRowsError::Structure)?;
    if let Some(parent) = stack.last_mut() {
        parent
            .header
            .element
            .children
            .push(completed.header.element);
    } else {
        *root = Some(completed.header.element);
    }
    Ok(())
}

struct Pending<'source> {
    id: u64,
    header: ElementHeader<'source>,
}

impl<'source> Pending<'source> {
    const fn attributes_complete(&self) -> bool {
        let element = &self.header.element;
        element.attributes.len() == self.header.attributes
            && element.syntax.attributes.len() == self.header.declarations
            && element.syntax.content.len() == self.header.contents
    }

    const fn complete(&self) -> bool {
        self.attributes_complete() && self.header.element.children.len() == self.header.children
    }

    const fn next_child(&self) -> MaterialElementParent {
        MaterialElementParent {
            element: self.id,
            position: self.header.element.children.len(),
        }
    }

    fn append(&mut self, row: MaterialElementRow<'source>) -> Result<(), MaterialElementRowsError> {
        let element = &mut self.header.element;
        match row {
            MaterialElementRow::DecodedAttribute { position, value }
                if position == element.attributes.len() && position < self.header.attributes =>
            {
                element.attributes.push(value);
            }
            MaterialElementRow::Declaration { position, value }
                if element.attributes.len() == self.header.attributes
                    && position == element.syntax.attributes.len()
                    && position < self.header.declarations =>
            {
                element.syntax.attributes.push(value);
            }
            MaterialElementRow::Content { position, value }
                if element.attributes.len() == self.header.attributes
                    && element.syntax.attributes.len() == self.header.declarations
                    && position == element.syntax.content.len()
                    && position < self.header.contents =>
            {
                element.syntax.content.push(value);
            }
            _ => return Err(MaterialElementRowsError::Structure),
        }
        Ok(())
    }
}
