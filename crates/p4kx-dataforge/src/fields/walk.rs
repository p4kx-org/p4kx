// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::walk_frame::{Action, Array, Frame, Owner, Structure};
use crate::fields::FieldKind;
use crate::fields::part::{Part, Stop, Walk};
use crate::fields::seen::Seen;
use crate::layout::{FieldSlot, Layout};
use crate::source_layout_index;
use crate::{DataForge, DataType, ValueKind};
use crate::{build, scalar};
use std::collections::HashSet;

impl<'source> Part<'_, 'source> {
    pub(super) fn structure(
        &mut self,
        forge: &'source DataForge<'source>,
        struct_index: u32,
        layout: &Layout<'source>,
        bytes: &'source [u8],
        node: u32,
        instance: u32,
    ) -> Walk {
        let owner = Owner {
            structure: struct_index,
            instance,
        };
        let mut inline = HashSet::from([(owner, struct_index)]);
        let root = Structure::new(*layout, bytes, node, owner, struct_index)?;
        let mut pending = vec![Frame::Structure(root)];
        while let Some(frame) = pending.last_mut() {
            if let Some(action) = frame.next(forge, self)? {
                if let Some(next) = self.action(forge, action, &mut inline)? {
                    pending.push(next);
                }
            } else {
                if let Some(identity) = frame.inline() {
                    inline.remove(&identity);
                }
                pending.pop();
            }
        }
        Ok(())
    }

    fn action(
        &mut self,
        forge: &'source DataForge<'source>,
        action: Action<'source>,
        inline: &mut HashSet<(Owner, u32)>,
    ) -> Result<Option<Frame<'source>>, Stop> {
        match action {
            Action::Field {
                slot,
                bytes,
                node,
                owner,
            } => {
                if slot.array {
                    return Ok(self.array(forge, slot, bytes, node));
                }
                if slot.data_type == DataType::Class {
                    self.leaf(
                        node,
                        FieldKind::new(DataType::Class, false),
                        Seen::Empty,
                        false,
                    );
                    return self.inline(forge, u32::from(slot.target), bytes, node, owner, inline);
                }
                self.single(forge, slot, bytes, node, false, inline)
            }
            Action::Single { slot, bytes, node } => {
                self.single(forge, slot, bytes, node, true, inline)
            }
            Action::Class {
                slot,
                element,
                node,
            } => self.class_element(forge, slot, element, node, inline),
        }
    }

    fn inline(
        &mut self,
        forge: &'source DataForge<'source>,
        structure: u32,
        bytes: &'source [u8],
        node: u32,
        owner: Owner,
        inline: &mut HashSet<(Owner, u32)>,
    ) -> Result<Option<Frame<'source>>, Stop> {
        if !inline.insert((owner, structure)) {
            self.skipped += 1;
            return Ok(None);
        }
        let layout = match self.layouts.get(forge, structure) {
            Ok(layout) => layout,
            Err(error) if source_layout_index::is_operational(&error) => return Err(error.into()),
            Err(_) => {
                self.skipped += 1;
                inline.remove(&(owner, structure));
                return Ok(None);
            }
        };
        Ok(Some(Frame::Structure(Structure::new(
            layout, bytes, node, owner, structure,
        )?)))
    }

    fn single(
        &mut self,
        forge: &'source DataForge<'source>,
        slot: FieldSlot<'source>,
        bytes: &'source [u8],
        node: u32,
        array: bool,
        inline: &mut HashSet<(Owner, u32)>,
    ) -> Result<Option<Frame<'source>>, Stop> {
        let kind = FieldKind::new(slot.data_type, array);
        match slot.data_type {
            DataType::StrongPointer => match build::pointer(bytes) {
                None => self.leaf(node, kind, Seen::Empty, true),
                Some((structure, instance)) => {
                    self.leaf(node, kind, Seen::Empty, false);
                    return self.owned(forge, structure, instance, node, inline);
                }
            },
            DataType::WeakPointer => match build::pointer(bytes) {
                None => self.leaf(node, kind, Seen::Empty, true),
                Some((structure, instance)) => {
                    let seen = if forge.instance(structure, instance).is_some() {
                        Seen::Empty
                    } else {
                        Seen::Unreadable
                    };
                    self.leaf(node, kind, seen, false);
                }
            },
            DataType::Reference => match build::reference_guid(bytes) {
                None => self.leaf(node, kind, Seen::Empty, true),
                Some(guid) => self.leaf(node, kind, Seen::reference(&guid), false),
            },
            data_type => {
                let seen = scalar::read_scalar(forge, data_type, slot.target, bytes)
                    .map_or(Seen::Unreadable, |value| Seen::of(data_type, &value));
                let empty = matches!(seen, Seen::Empty);
                self.leaf(node, kind, seen, empty);
            }
        }
        Ok(None)
    }

    fn owned(
        &mut self,
        forge: &'source DataForge<'source>,
        structure: u32,
        instance: u32,
        parent: u32,
        inline: &mut HashSet<(Owner, u32)>,
    ) -> Result<Option<Frame<'source>>, Stop> {
        let Some(bytes) = forge.instance(structure, instance) else {
            self.skipped += 1;
            return Ok(None);
        };
        if !self.visited.insert((structure, instance)) {
            self.skipped += 1;
            return Ok(None);
        }
        let layout = match self.layouts.get(forge, structure) {
            Ok(layout) => layout,
            Err(error) if source_layout_index::is_operational(&error) => return Err(error.into()),
            Err(_) => {
                self.skipped += 1;
                return Ok(None);
            }
        };
        let node = self.type_node(parent, structure, layout.type_name)?;
        self.leaf(
            node,
            FieldKind::new(DataType::Class, false),
            Seen::Empty,
            false,
        );
        let owner = Owner {
            structure,
            instance,
        };
        inline.insert((owner, structure));
        Ok(Some(Frame::Structure(Structure::new(
            layout, bytes, node, owner, structure,
        )?)))
    }

    fn array(
        &mut self,
        forge: &DataForge<'source>,
        slot: FieldSlot<'source>,
        bytes: &[u8],
        node: u32,
    ) -> Option<Frame<'source>> {
        let [count, first] = build::words(bytes);
        let kind = ValueKind::of(slot.data_type);
        let len = kind.map_or_else(
            || {
                forge
                    .instance_counts
                    .get(usize::from(slot.target))
                    .map_or(0, |count| u64::from(*count))
            },
            |kind| forge.value_count(kind) as u64,
        );
        if u64::from(first) + u64::from(count) > len {
            self.skipped += 1;
            return None;
        }
        Some(Frame::Array(Array {
            slot,
            kind,
            count,
            first,
            node,
            next: 0,
        }))
    }

    fn class_element(
        &mut self,
        forge: &'source DataForge<'source>,
        slot: FieldSlot<'source>,
        instance: u32,
        node: u32,
        inline: &mut HashSet<(Owner, u32)>,
    ) -> Result<Option<Frame<'source>>, Stop> {
        let structure = u32::from(slot.target);
        let Some(bytes) = forge.instance(structure, instance) else {
            self.skipped += 1;
            return Ok(None);
        };
        if !self.visited.insert((structure, instance)) {
            self.skipped += 1;
            return Ok(None);
        }
        self.leaf(
            node,
            FieldKind::new(DataType::Class, true),
            Seen::Empty,
            false,
        );
        self.inline(
            forge,
            structure,
            bytes,
            node,
            Owner {
                structure,
                instance,
            },
            inline,
        )
    }
}
