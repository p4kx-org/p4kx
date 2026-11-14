// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::slice;

use p4kx_extraction::{CancelToken, Value};

use crate::crafting_cost_read;
use crate::crafting_types::{Cost, CostIndex};
use crate::record_fields;
use crate::{CatalogError, ExportAccountingError};

/// One tier's semantic costs and the exact source occurrence of each node.
#[derive(Default)]
pub(super) struct RecipeCosts<'read, 'source> {
    pub(super) values: Vec<Cost<'source>>,
    pub(super) originals: Vec<&'read Value<'source>>,
}

struct Node<'read, 'source> {
    index: CostIndex,
    options: Option<slice::Iter<'read, Value<'source>>>,
}

struct Frame<'read, 'source> {
    index: CostIndex,
    options: slice::Iter<'read, Value<'source>>,
}

impl<'read, 'source> Node<'read, 'source> {
    fn frame(self) -> Option<Frame<'read, 'source>> {
        self.options.map(|options| Frame {
            index: self.index,
            options,
        })
    }
}

impl<'read, 'source> RecipeCosts<'read, 'source> {
    pub(super) fn read(
        &mut self,
        value: &'read Value<'source>,
        unknown: &mut Vec<&'source str>,
        cancel: Option<&CancelToken>,
    ) -> Result<Option<CostIndex>, CatalogError> {
        checkpoint(cancel)?;
        let Some(root) = self.add(value, unknown) else {
            return Ok(None);
        };
        let index = root.index;
        let mut stack: Vec<_> = root.frame().into_iter().collect();
        while let Some(frame) = stack.last_mut() {
            checkpoint(cancel)?;
            let Some(value) = frame.options.next() else {
                stack.pop();
                continue;
            };
            let parent = frame.index;
            if let Some(child) = self.add(value, unknown) {
                self.attach(parent, child.index)?;
                if let Some(frame) = child.frame() {
                    stack.push(frame);
                }
            }
        }
        Ok(Some(index))
    }

    fn add(
        &mut self,
        value: &'read Value<'source>,
        unknown: &mut Vec<&'source str>,
    ) -> Option<Node<'read, 'source>> {
        let Value::Struct(source) = value else {
            return None;
        };
        let cost = crafting_cost_read::read(source, unknown)?;
        let options = matches!(&cost, Cost::Select(_))
            .then(|| record_fields::elements(Some(source), "options").iter());
        let index = CostIndex::new(self.values.len());
        self.values.push(cost);
        self.originals.push(value);
        Some(Node { index, options })
    }

    fn attach(&mut self, parent: CostIndex, child: CostIndex) -> Result<(), CatalogError> {
        let Some(Cost::Select(selection)) = self.values.get_mut(parent.index()) else {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        };
        selection.options.push(child);
        Ok(())
    }
}

fn checkpoint(cancel: Option<&CancelToken>) -> Result<(), CatalogError> {
    if cancel.is_some_and(CancelToken::is_cancelled) {
        return Err(CatalogError::Cancelled);
    }
    Ok(())
}
