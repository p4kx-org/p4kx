// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::RawElement;
use crate::mtl_raw;
use crate::mtl_source::{
    MaterialEncoding, MaterialSource, MaterialSourceLimits, MaterialSourceLoss,
    MaterialSourceLossKind,
};
use crate::mtl_source_error::MaterialReadError;
use crate::mtl_xml::XmlElement;

pub(crate) fn capture<'input, Interrupted>(
    root: impl XmlElement<'input>,
    encoding: MaterialEncoding,
    repaired: bool,
    limits: MaterialSourceLimits,
    checkpoint: &mut impl FnMut() -> Result<(), Interrupted>,
) -> Result<MaterialSource, MaterialReadError<Interrupted>> {
    let mut capture = Capture {
        limits,
        remaining_elements: limits.elements,
        bytes: 0,
        losses: Vec::new(),
        checkpoint,
    };
    if repaired {
        capture.loss(MaterialSourceLossKind::RepairedEncoding, &[]);
    }
    let root = capture.element(root, None, &mut Vec::new())?;
    Ok(MaterialSource {
        encoding,
        root,
        losses: capture.losses,
        submaterial_list_ordinal: None,
        entries: Vec::new(),
    })
}

struct Capture<'checkpoint, Checkpoint> {
    limits: MaterialSourceLimits,
    remaining_elements: Option<usize>,
    bytes: usize,
    losses: Vec<MaterialSourceLoss>,
    checkpoint: &'checkpoint mut Checkpoint,
}

impl<Checkpoint> Capture<'_, Checkpoint> {
    fn loss(&mut self, kind: MaterialSourceLossKind, indices: &[usize]) {
        if !self.losses.iter().any(|loss| loss.kind == kind) {
            self.losses.push(MaterialSourceLoss {
                kind,
                indices: indices.to_vec(),
            });
        }
    }

    fn reserve(&mut self, bytes: Option<usize>, indices: &[usize]) -> bool {
        let Some(total) = bytes.and_then(|bytes| self.bytes.checked_add(bytes)) else {
            self.loss(MaterialSourceLossKind::TextBytes, indices);
            return false;
        };
        if total > self.limits.text_bytes {
            self.loss(MaterialSourceLossKind::TextBytes, indices);
            return false;
        }
        self.bytes = total;
        true
    }

    fn element<'input, Interrupted>(
        &mut self,
        element: impl XmlElement<'input>,
        ordinal: Option<usize>,
        indices: &mut Vec<usize>,
    ) -> Result<Option<RawElement>, MaterialReadError<Interrupted>>
    where
        Checkpoint: FnMut() -> Result<(), Interrupted>,
    {
        (self.checkpoint)().map_err(MaterialReadError::Interrupted)?;
        if self.remaining_elements == Some(0) {
            self.loss(MaterialSourceLossKind::Elements, indices);
            return Ok(None);
        }
        let syntax = element.syntax()?;
        let legacy_bytes =
            element
                .attributes()
                .try_fold(element.name().len(), |size, (name, value)| {
                    size.checked_add(size_of::<(String, String)>())?
                        .checked_add(name.len())?
                        .checked_add(value.len())
                });
        let bytes = syntax
            .owned_bytes()
            .and_then(|syntax| legacy_bytes?.checked_add(syntax));
        if !self.reserve(bytes, indices) {
            return Ok(None);
        }
        if let Some(remaining) = &mut self.remaining_elements {
            *remaining -= 1;
        }
        let mut children = Vec::new();
        let children_in_order = element.children().enumerate();
        for (position, child) in children_in_order {
            indices.push(position);
            if indices.len() > self.limits.depth {
                self.loss(MaterialSourceLossKind::Depth, indices);
                indices.pop();
                break;
            }
            if let Some(child) = self.element(child, Some(position), indices)? {
                children.push(child);
            }
            indices.pop();
            if self.remaining_elements == Some(0) {
                if element.children().nth(position + 1).is_some() {
                    self.loss(MaterialSourceLossKind::Elements, indices);
                }
                break;
            }
        }
        Ok(Some(RawElement {
            name: element.name().to_owned(),
            attributes: mtl_raw::attributes(element),
            children,
            authored: syntax.into_owned(ordinal),
        }))
    }
}
