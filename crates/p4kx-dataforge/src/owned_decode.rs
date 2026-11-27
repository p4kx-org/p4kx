// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;

use crate::layout::FieldSlot;
use crate::scalar;
use crate::{DataForge, DataForgeError, DataType, Value, ValueError};

use super::array::Array;
use super::bytes;
use super::frame::{Frame, Structure};
use super::positions::Position;
use super::task::{Input, Task};
use super::tree_builder::{Pending, TreeBuilder};

pub(super) enum Decoded<'source> {
    Value(Value<'source>),
    Frame(Frame<'source>),
}

pub(super) struct Decoder<'reader, 'source> {
    builder: &'reader mut TreeBuilder<'source>,
    forge: &'source DataForge<'source>,
    inline: HashSet<(Position, u32)>,
}

impl<'reader, 'source> Decoder<'reader, 'source> {
    pub(super) fn new(
        builder: &'reader mut TreeBuilder<'source>,
        forge: &'source DataForge<'source>,
        root_struct: u32,
    ) -> Self {
        Self {
            builder,
            forge,
            inline: HashSet::from([(Position::ROOT, root_struct)]),
        }
    }

    pub(super) fn read(&mut self, task: Task<'source>) -> Result<Decoded<'source>, DataForgeError> {
        match task.input {
            Input::Field { slot, bytes } if slot.array => self.array(task, slot, bytes),
            Input::Field { slot, bytes } if slot.data_type == DataType::Class => {
                let position = task.position(&mut self.builder.positions)?;
                self.structure(u32::from(slot.target), bytes, position, task.group)
            }
            Input::Field { slot, bytes } => self.single(task, slot.data_type, slot.target, bytes),
            Input::Single {
                data_type,
                target,
                bytes,
            } => self.single(task, data_type, target, bytes),
            Input::Owned {
                struct_index,
                instance_index,
            } => self.owned(task, struct_index, instance_index),
        }
    }

    #[cfg(test)]
    pub(super) fn measure_depth(&mut self, frames: usize) {
        self.builder.peak_frames = self.builder.peak_frames.max(frames);
    }
    pub(super) fn leave(&mut self, inline: (Position, u32)) {
        self.inline.remove(&inline);
    }

    fn single(
        &mut self,
        task: Task<'source>,
        data_type: DataType,
        target: u16,
        bytes: &'source [u8],
    ) -> Result<Decoded<'source>, DataForgeError> {
        let value = match data_type {
            DataType::StrongPointer => match bytes::pointer(bytes) {
                Some((structure, instance)) => return self.owned(task, structure, instance),
                None => Value::Null,
            },
            DataType::WeakPointer => self.weak(task, bytes)?,
            DataType::Reference => bytes::reference(self.forge, bytes),
            other => scalar::read_scalar(self.forge, other, target, bytes).unwrap_or(Value::Null),
        };
        Ok(Decoded::Value(value))
    }

    fn structure(
        &mut self,
        struct_index: u32,
        bytes: &'source [u8],
        position: Position,
        group: Position,
    ) -> Result<Decoded<'source>, DataForgeError> {
        if !self.inline.insert((group, struct_index)) {
            return Ok(Decoded::Value(Value::Error(ValueError::InlineClassCycle {
                struct_index,
            })));
        }
        let layout = self.builder.layouts.get(self.forge, struct_index)?;
        Ok(Decoded::Frame(Frame::Structure(Structure::new(
            layout,
            bytes,
            position,
            group,
            struct_index,
            &mut self.builder.materialization,
        )?)))
    }

    fn owned(
        &mut self,
        task: Task<'source>,
        struct_index: u32,
        instance_index: u32,
    ) -> Result<Decoded<'source>, DataForgeError> {
        let Some(bytes) = self.forge.instance(struct_index, instance_index) else {
            return Ok(Decoded::Value(Value::Error(ValueError::InvalidPointer {
                struct_index,
                instance_index,
            })));
        };
        let target = (struct_index, instance_index);
        if self.builder.owned.contains_key(&target) {
            return Ok(Decoded::Value(Value::Error(ValueError::OwnedTwice {
                struct_index,
                instance_index,
            })));
        }
        let position = task.position(&mut self.builder.positions)?;
        self.builder.owned.insert(target, position);
        self.structure(struct_index, bytes, position, position)
    }

    fn weak(
        &mut self,
        task: Task<'source>,
        bytes: &[u8],
    ) -> Result<Value<'source>, DataForgeError> {
        let Some(target) = bytes::pointer(bytes) else {
            return Ok(Value::Null);
        };
        if self.forge.instance(target.0, target.1).is_none() {
            return Ok(Value::Error(ValueError::InvalidPointer {
                struct_index: target.0,
                instance_index: target.1,
            }));
        }
        let at = task.position(&mut self.builder.positions)?;
        self.builder.weak.push(Pending { target, at });
        Ok(Value::Null)
    }

    fn array(
        &mut self,
        task: Task<'source>,
        slot: FieldSlot<'source>,
        bytes: &[u8],
    ) -> Result<Decoded<'source>, DataForgeError> {
        let array = Array::new(
            self.forge,
            slot,
            bytes,
            task,
            &mut self.builder.positions,
            &mut self.builder.materialization,
        )?;
        Ok(match array {
            Ok(frame) => Decoded::Frame(Frame::Array(frame)),
            Err(error) => Decoded::Value(Value::Error(error)),
        })
    }
}
