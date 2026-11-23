// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::walk_frame::{Action, Array, Frame, Owner, Site, Structure};
use crate::fields::FieldKind;
use crate::fields::observation::FieldStatisticsInput;
use crate::fields::part::{Part, Stop, Walk};
use crate::fields::seen::Seen;
use crate::fields::{FieldStatisticsGap, FieldStatisticsScalarRead, FieldStatisticsUse};
use crate::layout::{FieldSlot, Layout};
use crate::source_layout_index;
use crate::{DataForge, DataType, Value, ValueKind};
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
                let source = action.selector();
                if let Some(next) = self.action(forge, action, &mut inline)? {
                    if let Some(source) = source {
                        self.routes.enter(source, next.source_target());
                    }
                    pending.push(next);
                }
            } else {
                if let Some(identity) = frame.inline() {
                    inline.remove(&identity);
                }
                pending.pop();
                self.routes.leave();
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
                site,
                owner,
            } => {
                if slot.array {
                    return Ok(self.array(forge, slot, bytes, site));
                }
                if slot.data_type == DataType::Class {
                    self.leaf(
                        &site,
                        FieldKind::new(DataType::Class, false),
                        Seen::Empty,
                        false,
                        None,
                    );
                    return self.inline(forge, u32::from(slot.target), bytes, &site, owner, inline);
                }
                self.single(forge, slot, bytes, &site, false, inline)
            }
            Action::Single { slot, bytes, site } => {
                self.single(forge, slot, bytes, &site, true, inline)
            }
            Action::Class {
                slot,
                element,
                site,
            } => self.class_element(forge, slot, element, &site, inline),
        }
    }

    fn inline(
        &mut self,
        forge: &'source DataForge<'source>,
        structure: u32,
        bytes: &'source [u8],
        site: &Site,
        owner: Owner,
        inline: &mut HashSet<(Owner, u32)>,
    ) -> Result<Option<Frame<'source>>, Stop> {
        if !inline.insert((owner, structure)) {
            self.skip(site, FieldStatisticsGap::RevisitedInstance);
            return Ok(None);
        }
        let layout = match self.layouts.get(forge, structure) {
            Ok(layout) => layout,
            Err(error) if source_layout_index::is_operational(&error) => return Err(error.into()),
            Err(_) => {
                self.skip(site, FieldStatisticsGap::UnreadableLayout);
                inline.remove(&(owner, structure));
                return Ok(None);
            }
        };
        self.observe_instance(forge, site, structure, owner.instance, bytes, None);
        Ok(Some(Frame::Structure(Structure::new(
            layout, bytes, site.node, owner, structure,
        )?)))
    }

    fn single(
        &mut self,
        forge: &'source DataForge<'source>,
        slot: FieldSlot<'source>,
        bytes: &'source [u8],
        site: &Site,
        array: bool,
        inline: &mut HashSet<(Owner, u32)>,
    ) -> Result<Option<Frame<'source>>, Stop> {
        let kind = FieldKind::new(slot.data_type, array);
        match slot.data_type {
            DataType::StrongPointer => match build::pointer(bytes) {
                None => self.leaf(site, kind, Seen::Empty, true, None),
                Some((structure, instance)) => {
                    self.leaf(site, kind, Seen::Empty, false, None);
                    return self.owned(forge, structure, instance, site, inline);
                }
            },
            DataType::WeakPointer => match build::pointer(bytes) {
                None => self.leaf(site, kind, Seen::Empty, true, None),
                Some((structure, instance)) => {
                    let seen =
                        forge
                            .instance(structure, instance)
                            .map_or(Seen::Unreadable, |bytes| {
                                self.observe_instance(
                                    forge, site, structure, instance, bytes, None,
                                );
                                Seen::Empty
                            });
                    self.leaf(site, kind, seen, false, None);
                }
            },
            DataType::Reference => match build::reference_guid(bytes) {
                None => self.leaf(site, kind, Seen::Empty, true, None),
                Some(guid) => self.leaf(site, kind, Seen::reference(&guid), false, None),
            },
            data_type => {
                let mut scalar_read = FieldStatisticsScalarRead::default();
                let value = if site.input.is_some() {
                    scalar::read_scalar_with(
                        forge,
                        data_type,
                        slot.target,
                        bytes,
                        Some(&mut |source| scalar_read.observe(source)),
                    )
                } else {
                    scalar::read_scalar(forge, data_type, slot.target, bytes)
                };
                let seen = value
                    .as_ref()
                    .map_or(Seen::Unreadable, |value| Seen::of(data_type, value));
                let empty = matches!(seen, Seen::Empty);
                let text = match (site.input.is_some(), value) {
                    (true, Some(Value::String(text) | Value::Locale(text) | Value::Enum(text))) => {
                        forge.original_text_span(text)
                    }
                    _ => None,
                };
                self.leaf_with_sources(site, kind, seen, empty, text, scalar_read);
            }
        }
        Ok(None)
    }

    fn owned(
        &mut self,
        forge: &'source DataForge<'source>,
        structure: u32,
        instance: u32,
        site: &Site,
        inline: &mut HashSet<(Owner, u32)>,
    ) -> Result<Option<Frame<'source>>, Stop> {
        let Some(bytes) = forge.instance(structure, instance) else {
            self.skip(site, FieldStatisticsGap::MissingInstance);
            return Ok(None);
        };
        if !self.visited.insert((structure, instance)) {
            self.skip(site, FieldStatisticsGap::RevisitedInstance);
            return Ok(None);
        }
        let layout = match self.layouts.get(forge, structure) {
            Ok(layout) => layout,
            Err(error) if source_layout_index::is_operational(&error) => return Err(error.into()),
            Err(_) => {
                self.skip(site, FieldStatisticsGap::UnreadableLayout);
                return Ok(None);
            }
        };
        let node = self.type_node(site.node, structure, layout.type_name)?;
        let target = Site {
            node,
            input: site.input.clone(),
        };
        self.observe_instance(
            forge,
            &target,
            structure,
            instance,
            bytes,
            Some(layout.type_name),
        );
        self.leaf(
            &target,
            FieldKind::new(DataType::Class, false),
            Seen::Empty,
            false,
            None,
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

    fn observe_instance(
        &self,
        forge: &DataForge<'_>,
        site: &Site,
        structure: u32,
        instance: u32,
        bytes: &[u8],
        name: Option<&str>,
    ) {
        if site.input.is_some() {
            self.observe_site(
                site,
                FieldStatisticsUse::Instance {
                    structure,
                    instance,
                    bytes: forge.byte_span(bytes),
                    path_name: name.and_then(|name| forge.original_text_span(name)),
                },
            );
        }
    }

    fn array(
        &mut self,
        forge: &DataForge<'source>,
        slot: FieldSlot<'source>,
        bytes: &[u8],
        site: Site,
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
            self.skip(&site, FieldStatisticsGap::InvalidArrayExtent);
            return None;
        }
        let header = match site.input {
            Some(FieldStatisticsInput::Slot(header)) => Some(header),
            _ => None,
        };
        Some(Frame::Array(Array {
            slot,
            kind,
            count,
            first,
            node: site.node,
            next: 0,
            header,
        }))
    }

    fn class_element(
        &mut self,
        forge: &'source DataForge<'source>,
        slot: FieldSlot<'source>,
        instance: u32,
        site: &Site,
        inline: &mut HashSet<(Owner, u32)>,
    ) -> Result<Option<Frame<'source>>, Stop> {
        let structure = u32::from(slot.target);
        let Some(bytes) = forge.instance(structure, instance) else {
            self.skip(site, FieldStatisticsGap::MissingInstance);
            return Ok(None);
        };
        if !self.visited.insert((structure, instance)) {
            self.skip(site, FieldStatisticsGap::RevisitedInstance);
            return Ok(None);
        }
        self.leaf(
            site,
            FieldKind::new(DataType::Class, true),
            Seen::Empty,
            false,
            None,
        );
        self.inline(
            forge,
            structure,
            bytes,
            site,
            Owner {
                structure,
                instance,
            },
            inline,
        )
    }
}
