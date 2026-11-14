// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::layout::{Layout, Layouts};
use crate::layout_cursor::LayoutCursor;
use crate::source_layout_index;
use crate::{DataForge, DataForgeError, DataType};
use std::collections::HashMap;
use std::sync::Arc;

const MAX_STRUCT_POINTERS: usize = 1 << 16;

#[derive(Default)]
pub(crate) struct PointerOffsets<'source> {
    layouts: Layouts<'source>,
    done: HashMap<u32, Option<Arc<[usize]>>>,
}

struct Frame<'source> {
    index: u32,
    cursor: LayoutCursor<'source>,
    waiting: Option<(u32, usize)>,
    offsets: Vec<usize>,
    failed: bool,
}

enum Action<'source> {
    Continue,
    Child(Frame<'source>),
    Finish,
}

impl<'source> Frame<'source> {
    fn new(index: u32, layout: Layout<'source>) -> Result<Self, DataForgeError> {
        Ok(Self {
            index,
            cursor: layout.cursor()?,
            waiting: None,
            offsets: Vec::new(),
            failed: false,
        })
    }

    fn extend(&mut self, offsets: Option<&[usize]>, start: usize) {
        let Some(offsets) = offsets else {
            self.failed = true;
            return;
        };
        for offset in offsets {
            let Some(offset) = start.checked_add(*offset) else {
                self.failed = true;
                return;
            };
            self.offsets.push(offset);
            if self.offsets.len() > MAX_STRUCT_POINTERS {
                self.failed = true;
                return;
            }
        }
    }
}

impl<'source> PointerOffsets<'source> {
    pub(crate) fn of(
        &mut self,
        forge: &'source DataForge<'source>,
        index: u32,
    ) -> Result<Option<Arc<[usize]>>, DataForgeError> {
        let result = self.read(forge, index);
        if result.is_err() {
            self.done.clear();
        }
        result
    }

    fn read(
        &mut self,
        forge: &'source DataForge<'source>,
        index: u32,
    ) -> Result<Option<Arc<[usize]>>, DataForgeError> {
        if let Some(known) = self.done.get(&index) {
            return Ok(known.clone());
        }
        self.done.insert(index, None);
        let layout = match self.layouts.get(forge, index) {
            Ok(layout) => layout,
            Err(error) if source_layout_index::is_operational(&error) => return Err(error),
            Err(_) => return Ok(None),
        };
        let mut pending = vec![Frame::new(index, layout)?];
        while let Some(mut frame) = pending.pop() {
            match self.advance(forge, &mut frame)? {
                Action::Continue => pending.push(frame),
                Action::Child(child) => {
                    pending.push(frame);
                    pending.push(child);
                }
                Action::Finish => {
                    let found = (!frame.failed).then(|| Arc::from(frame.offsets));
                    self.done.insert(frame.index, found);
                }
            }
        }
        Ok(self.done.get(&index).cloned().flatten())
    }

    fn advance(
        &mut self,
        forge: &'source DataForge<'source>,
        frame: &mut Frame<'source>,
    ) -> Result<Action<'source>, DataForgeError> {
        if let Some((child, start)) = frame.waiting.take() {
            frame.extend(self.done.get(&child).and_then(Option::as_deref), start);
        }
        if frame.offsets.len() > MAX_STRUCT_POINTERS {
            frame.failed = true;
        }
        if frame.failed {
            return Ok(Action::Finish);
        }
        let Some(slot) = frame.cursor.next_slot()? else {
            return Ok(Action::Finish);
        };
        if slot.array {
            return Ok(Action::Continue);
        }
        match slot.data_type {
            DataType::StrongPointer | DataType::WeakPointer => frame.offsets.push(slot.start),
            DataType::Class if slot.end > slot.start => {
                let child = u32::from(slot.target);
                if let Some(known) = self.done.get(&child) {
                    frame.extend(known.as_deref(), slot.start);
                } else {
                    self.done.insert(child, None);
                    match self.layouts.get(forge, child) {
                        Ok(layout) => {
                            frame.waiting = Some((child, slot.start));
                            return Ok(Action::Child(Frame::new(child, layout)?));
                        }
                        Err(error) if source_layout_index::is_operational(&error) => {
                            return Err(error);
                        }
                        Err(_) => frame.failed = true,
                    }
                }
            }
            _ => {}
        }
        Ok(Action::Continue)
    }
}
