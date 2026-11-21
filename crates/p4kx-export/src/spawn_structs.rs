// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::slice;

use p4kx_extraction::{StructValue, Value};

use super::ReadContext;
use crate::record_read::ReadError;

pub(super) struct Structs<'context, 'parent, 'source> {
    context: Option<&'context dyn ReadContext<'source>>,
    items: slice::Iter<'parent, Value<'source>>,
    stopped: bool,
}

impl<'context, 'parent, 'source> Structs<'context, 'parent, 'source> {
    pub(super) fn new(
        items: &'parent [Value<'source>],
        context: Option<&'context dyn ReadContext<'source>>,
    ) -> Self {
        Self {
            context,
            items: items.iter(),
            stopped: false,
        }
    }
}

impl<'parent, 'source> Iterator for Structs<'_, 'parent, 'source> {
    type Item = Result<&'parent StructValue<'source>, ReadError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.stopped {
            return None;
        }
        for value in self.items.by_ref() {
            if let Some(context) = self.context
                && let Err(error) = context.element(value)
            {
                self.stopped = true;
                return Some(Err(error));
            }
            if let Value::Struct(source) = value {
                return Some(Ok(source));
            }
        }
        self.stopped = true;
        None
    }
}
