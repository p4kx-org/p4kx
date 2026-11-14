// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::layout::Layout;
use crate::positioned_record::InstanceSpans;
use crate::{DataForge, DataForgeError, Field, Value};

use super::TreeBuilder;
use super::decode::{Decoded, Decoder};
use super::frame::{Action, Frame, Structure};
use super::positions::Position;

pub(super) fn fields<'source>(
    builder: &mut TreeBuilder<'source>,
    forge: &'source DataForge<'source>,
    struct_index: u32,
    layout: Layout<'source>,
    bytes: &'source [u8],
    mut instances: Option<&mut InstanceSpans<'source>>,
) -> Result<Vec<Field<'source>>, DataForgeError> {
    let root = Structure::new(
        layout,
        bytes,
        Position::ROOT,
        Position::ROOT,
        struct_index,
        &mut builder.materialization,
    )?;
    let mut frames = vec![Frame::Structure(root)];
    let mut reader = Decoder::new(builder, forge, struct_index);
    #[cfg(test)]
    reader.measure_depth(frames.len());
    loop {
        let action = frames
            .last_mut()
            .ok_or(DataForgeError::CanonicalTreeInvariant)?
            .next(forge)?;
        match action {
            Action::Read(task) => match reader.read(task)? {
                Decoded::Value(value) => accept(&mut frames, value)?,
                Decoded::Frame(frame) => {
                    frames.push(frame);
                    #[cfg(test)]
                    reader.measure_depth(frames.len());
                }
            },
            Action::Complete => {
                let completed = frames
                    .pop()
                    .ok_or(DataForgeError::CanonicalTreeInvariant)?
                    .finish(instances.as_deref_mut())?;
                if let Some(inline) = completed.inline {
                    reader.leave(inline);
                }
                if frames.is_empty() {
                    let Value::Struct(root) = completed.value else {
                        return Err(DataForgeError::CanonicalTreeInvariant);
                    };
                    return Ok(root.into_fields());
                }
                accept(&mut frames, completed.value)?;
            }
        }
    }
}

fn accept<'source>(
    frames: &mut [Frame<'source>],
    value: Value<'source>,
) -> Result<(), DataForgeError> {
    frames
        .last_mut()
        .ok_or(DataForgeError::CanonicalTreeInvariant)?
        .accept(value)
}
