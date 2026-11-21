// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::Range;

use p4kx_extraction::ConversionError;

use super::super::{Check, Visit};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum VectorRead {
    Present,
    Absent,
    Failed,
}

#[derive(Debug, Default)]
pub(super) struct VectorReads {
    paths: RefCell<HashMap<String, VectorRead>>,
}

#[derive(Debug)]
pub(super) struct Frame {
    pub(super) items: Range<usize>,
    pub(super) vector: VectorRead,
}

impl VectorReads {
    pub(super) fn record(&self, path: &str, read: &Result<Vec<u8>, ConversionError>) {
        let status = match read {
            Ok(_) => VectorRead::Present,
            Err(ConversionError::UnknownEntry(_)) => VectorRead::Absent,
            Err(_) => VectorRead::Failed,
        };
        self.paths
            .borrow_mut()
            .insert(path.to_ascii_lowercase(), status);
    }

    pub(super) fn frame(&self, visit: &Visit) -> Check<Frame> {
        let [requested] = visit.vector_requests.as_slice() else {
            return Err("the original template proxy did not request exactly one SVG");
        };
        if visit.styled_vector.as_ref() != Some(requested) {
            return Err("the original template proxy did not request its styled SVG");
        }
        let vector = self
            .paths
            .borrow()
            .get(&requested.to_ascii_lowercase())
            .copied()
            .ok_or("the original SVG request has no Archive result")?;
        let vectors = visit
            .vectors
            .as_deref()
            .ok_or("the original template proxy was painted without input receipts")?;
        let items = match (vector, vectors) {
            (VectorRead::Present, [(item, path)]) if path == requested => *item..*item + 1,
            (VectorRead::Present, _) => {
                return Err("the original template proxy did not paint exactly its styled SVG");
            }
            (VectorRead::Absent, []) if !visit.own_items.is_empty() => visit.own_items.clone(),
            (VectorRead::Absent, _) => {
                return Err("the template proxy with a missing SVG did not paint only its box");
            }
            (VectorRead::Failed, _) => return Err("the original SVG could not be read"),
        };
        Ok(Frame { items, vector })
    }
}
