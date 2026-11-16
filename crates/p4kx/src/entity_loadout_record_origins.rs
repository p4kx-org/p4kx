// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::ControlFlow;
use std::ptr;

use p4kx_assembly::{LoadoutPathObserver, LoadoutPathOperation, LoadoutSelection};
use p4kx_extraction::{
    DataForge, Record, RecordJsonLocation, RecordTree, SourceLocation, StructValue, Value,
};

pub(super) struct RecordPaths<'forge> {
    forge: &'forge DataForge<'forge>,
    selections: RefCell<HashMap<usize, Vec<Pending>>>,
}

struct Pending {
    selection: LoadoutSelection,
    text: Option<SourceLocation>,
}

pub(super) struct SelectedName {
    pub(super) record: u32,
    pub(super) ordinal: u64,
    pub(super) text: Option<SourceLocation>,
}

impl<'forge> RecordPaths<'forge> {
    pub(super) fn new(forge: &'forge DataForge<'forge>) -> Self {
        Self {
            forge,
            selections: RefCell::new(HashMap::new()),
        }
    }

    pub(super) fn positions(
        self,
        record: &Record<'_>,
        tree: &RecordTree<'_>,
    ) -> Vec<(LoadoutSelection, SelectedName)> {
        let mut pending = self.selections.into_inner();
        let mut selected = Vec::new();
        if !pending.is_empty() {
            tree.root().visit_owned_values(|visit| {
                let address = ptr::from_ref(visit.value()).addr();
                if let Some(sites) = pending.remove(&address) {
                    selected.extend(sites.into_iter().map(|site| {
                        (
                            site.selection,
                            SelectedName {
                                record: record.index(),
                                ordinal: visit.ordinal() as u64,
                                text: site.text,
                            },
                        )
                    }));
                }
            });
        }
        selected
    }
}

impl LoadoutPathObserver for RecordPaths<'_> {
    fn record_path(
        &self,
        selection: &LoadoutSelection,
        parent: &StructValue<'_>,
        value: &Value<'_>,
    ) -> ControlFlow<()> {
        if selection.operation() != LoadoutPathOperation::PortName
            || !parent
                .fields()
                .iter()
                .any(|field| ptr::eq(field.value(), value))
        {
            return ControlFlow::Continue(());
        }
        let text = match value {
            Value::String(text) | Value::Locale(text) | Value::Enum(text) => {
                RecordJsonLocation::borrowed_text(self.forge, text)
            }
            _ => None,
        };
        self.selections
            .borrow_mut()
            .entry(ptr::from_ref(value).addr())
            .or_default()
            .push(Pending {
                selection: selection.clone(),
                text,
            });
        ControlFlow::Continue(())
    }

    fn record_name(&self, _: &LoadoutSelection, _: &RecordTree<'_>) -> ControlFlow<()> {
        ControlFlow::Continue(())
    }
}
