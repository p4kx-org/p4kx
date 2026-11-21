// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::ops::ControlFlow;
use std::ptr;

use p4kx_assembly::{
    LoadoutClassSelection, LoadoutPathObserver, LoadoutPathOperation, LoadoutRecordText,
    LoadoutReferenceSelection, LoadoutSelection, PortDefinition,
};
use p4kx_extraction::{
    DataForge, DataType, Guid, Record, RecordJsonLocation, RecordText, RecordTree, SourceLocation,
    StructValue, Value, ValueError,
};

use super::definition_sites::{Definition, DefinitionKey, PendingDefinition, Site};

pub(super) struct RecordPaths<'forge> {
    forge: &'forge DataForge<'forge>,
    selections: RefCell<HashMap<usize, Vec<Pending>>>,
    definitions: RefCell<Vec<PendingDefinition>>,
    names: RefCell<Vec<(LoadoutSelection, RecordName)>>,
}

pub(super) struct CapturedRecord {
    pub(super) selections: Vec<(Selection, RecordSite)>,
    pub(super) definitions: Vec<(DefinitionKey, Definition)>,
    pub(super) names: Vec<(LoadoutSelection, RecordName)>,
}

pub(super) struct RecordName {
    pub(super) record: u32,
    pub(super) raw: Option<SourceLocation>,
    pub(super) complete: bool,
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
    Text(LoadoutRecordText),
    Reference { guid: Guid, stored: bool },
}

impl<'forge> RecordPaths<'forge> {
    pub(super) fn new(forge: &'forge DataForge<'forge>) -> Self {
        Self {
            forge,
            selections: RefCell::new(HashMap::new()),
            definitions: RefCell::new(Vec::new()),
            names: RefCell::new(Vec::new()),
        }
    }

    pub(super) fn positions(self, record: &Record<'_>, tree: &RecordTree<'_>) -> CapturedRecord {
        let mut pending = self.selections.into_inner();
        let definitions = self.definitions.into_inner();
        let mut requested: HashSet<_> = definitions
            .iter()
            .flat_map(PendingDefinition::sites)
            .collect();
        let mut definition_sites = HashMap::new();
        let mut selections = Vec::new();
        if !pending.is_empty() || !requested.is_empty() {
            tree.root().visit_owned_values(|visit| {
                let value = visit.value();
                let ordinal = visit.ordinal() as u64;
                let address = ptr::from_ref(value).addr();
                if let Some(sites) = pending.remove(&address) {
                    selections.extend(sites.into_iter().map(|site| {
                        (
                            site.selection,
                            RecordSite {
                                record: record.index(),
                                ordinal,
                                datum: site.datum,
                            },
                        )
                    }));
                }
                let site = Site::value(value);
                if requested.remove(&site) {
                    definition_sites.insert(site, ordinal);
                }
                if let Value::Struct(structure) = value {
                    let site = Site::structure(structure);
                    if requested.remove(&site) {
                        definition_sites.insert(site, ordinal);
                    }
                }
            });
        }
        let definitions = definitions
            .into_iter()
            .map(|definition| definition.bind(&definition_sites))
            .collect();
        CapturedRecord {
            selections,
            definitions,
            names: self.names.into_inner(),
        }
    }

    fn capture_text(&self, selection: Selection, parent: &StructValue<'_>, value: &Value<'_>) {
        if let Some(text) = LoadoutRecordText::capture(self.forge, parent, value) {
            self.capture(selection, value, Datum::Text(text));
        }
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
            LoadoutPathOperation::PortName
                | LoadoutPathOperation::Copy
                | LoadoutPathOperation::Trim
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

    fn record_port_definition(
        &self,
        definition: &PortDefinition,
        original: &StructValue<'_>,
    ) -> ControlFlow<()> {
        self.definitions
            .borrow_mut()
            .push(PendingDefinition::read(self.forge, definition, original));
        ControlFlow::Continue(())
    }

    fn record_name(&self, selection: &LoadoutSelection, tree: &RecordTree<'_>) -> ControlFlow<()> {
        let (raw, complete) = match tree.metadata().name() {
            RecordText::Present(text) => {
                let site = RecordJsonLocation::borrowed_text(self.forge, text);
                let complete = site.is_some();
                (site, complete)
            }
            RecordText::Absent | RecordText::Null => (None, true),
            _ => (None, false),
        };
        self.names.borrow_mut().push((
            selection.clone(),
            RecordName {
                record: tree.record_index(),
                raw,
                complete,
            },
        ));
        ControlFlow::Continue(())
    }
}
