// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::ControlFlow;
use std::ptr;

use p4kx_assembly::{
    LoadoutClassSelection, LoadoutPathObserver, LoadoutPathOperation, LoadoutReferenceSelection,
    LoadoutSelection,
};
use p4kx_extraction::{
    DataForge, DataType, Guid, Record, RecordJsonLocation, RecordTree, SourceLocation, StructValue,
    Value, ValueError,
};

pub(super) struct RecordPaths<'forge> {
    forge: &'forge DataForge<'forge>,
    selections: RefCell<HashMap<usize, Vec<Pending>>>,
}

pub(super) enum Selection {
    Path(LoadoutSelection),
    Class(LoadoutClassSelection),
    Reference(LoadoutReferenceSelection),
}

struct Pending {
    selection: Selection,
    datum: Datum,
}

pub(super) struct RecordSite {
    pub(super) record: u32,
    pub(super) ordinal: u64,
    pub(super) datum: Datum,
}

pub(super) enum Datum {
    Text(Option<SourceLocation>),
    Reference { guid: Guid, stored: bool },
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
    ) -> Vec<(Selection, RecordSite)> {
        let mut pending = self.selections.into_inner();
        let mut selected = Vec::new();
        if !pending.is_empty() {
            tree.root().visit_owned_values(|visit| {
                let address = ptr::from_ref(visit.value()).addr();
                if let Some(sites) = pending.remove(&address) {
                    selected.extend(sites.into_iter().map(|site| {
                        (
                            site.selection,
                            RecordSite {
                                record: record.index(),
                                ordinal: visit.ordinal() as u64,
                                datum: site.datum,
                            },
                        )
                    }));
                }
            });
        }
        selected
    }

    fn capture_text(&self, selection: Selection, parent: &StructValue<'_>, value: &Value<'_>) {
        if !parent
            .fields()
            .iter()
            .any(|field| ptr::eq(field.value(), value))
        {
            return;
        }
        let location = match value {
            Value::String(text) | Value::Locale(text) | Value::Enum(text) => {
                RecordJsonLocation::borrowed_text(self.forge, text)
            }
            _ => None,
        };
        self.capture(selection, value, Datum::Text(location));
    }

    fn capture(&self, selection: Selection, value: &Value<'_>, datum: Datum) {
        self.selections
            .borrow_mut()
            .entry(ptr::from_ref(value).addr())
            .or_default()
            .push(Pending { selection, datum });
    }
}

impl LoadoutPathObserver for RecordPaths<'_> {
    fn record_path(
        &self,
        selection: &LoadoutSelection,
        parent: &StructValue<'_>,
        value: &Value<'_>,
    ) -> ControlFlow<()> {
        if matches!(
            selection.operation(),
            LoadoutPathOperation::PortName | LoadoutPathOperation::Copy
        ) {
            self.capture_text(Selection::Path(selection.clone()), parent, value);
        }
        ControlFlow::Continue(())
    }

    fn record_class(
        &self,
        selection: &LoadoutClassSelection,
        parent: &StructValue<'_>,
        value: &Value<'_>,
    ) -> ControlFlow<()> {
        self.capture_text(Selection::Class(selection.clone()), parent, value);
        ControlFlow::Continue(())
    }

    fn record_reference(
        &self,
        selection: &LoadoutReferenceSelection,
        parent: &StructValue<'_>,
        value: &Value<'_>,
    ) -> ControlFlow<()> {
        let Some(field) = parent
            .fields()
            .iter()
            .find(|field| ptr::eq(field.value(), value))
        else {
            return ControlFlow::Continue(());
        };
        let guid = match value {
            Value::Reference(reference) => reference.guid(),
            Value::Error(ValueError::DanglingReference { guid }) => *guid,
            _ => return ControlFlow::Continue(()),
        };
        self.capture(
            Selection::Reference(selection.clone()),
            value,
            Datum::Reference {
                guid,
                stored: field.storage().data_type() == DataType::Reference,
            },
        );
        ControlFlow::Continue(())
    }

    fn record_name(&self, _: &LoadoutSelection, _: &RecordTree<'_>) -> ControlFlow<()> {
        ControlFlow::Continue(())
    }
}
