// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::model::RawElement;
use p4kx_extraction::{MaterialElementRow, MaterialElementWriteError};

use super::{Output, prepared};
use crate::material_parts::{MaterialPart, MaterialTree};
use crate::material_parts_origins::ElementOrigins;
use crate::{MediaError, MediaWriter};

impl Output<'_, '_> {
    pub(super) fn tree(
        &mut self,
        tree: MaterialTree,
        element: &RawElement,
        root: Option<&[u64]>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let mut original = Vec::new();
        MaterialElementRow::visit(element, |path, part| {
            writer.checkpoint()?;
            original.clear();
            let actual = if let Some((root, path)) = root.zip(path) {
                original.extend_from_slice(root);
                for position in path {
                    original.push(crate::shader_values::count(*position)?);
                }
                Some(original.as_slice())
            } else {
                None
            };
            let mut origins = prepared(self.context, writer)?;
            ElementOrigins {
                input: &self.input,
                original: actual,
                encoding: self.encoding,
                writer,
            }
            .element(&mut origins, &part.row)?;
            self.emit(&MaterialPart::Element { tree, part }, origins, writer)
        })
        .map_err(|error| match error {
            MaterialElementWriteError::Interrupted(error) => error,
            _ => MediaError::Contract("material element count overflow".into()),
        })
    }
}
