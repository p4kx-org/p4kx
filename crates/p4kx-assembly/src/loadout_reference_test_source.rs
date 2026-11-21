// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::ControlFlow;
use std::ptr;
use std::sync::Mutex;

use p4kx_dataforge::{Field, Guid, Record, RecordTree, StructValue, Value, ValueError};
use p4kx_testkit::{self, DataForgeBuilder, TestEntity, TestLoadout, TestLoadoutEntry, TestPort};

use crate::{
    LoadoutPathObserver, LoadoutReadEvent, LoadoutReadObserver, LoadoutReferenceSelection,
    LoadoutSelection,
};

pub(super) type TestResult = Result<(), Box<dyn std::error::Error>>;

#[derive(Clone, Copy)]
pub(super) enum Kind {
    Manual,
    Default,
    Assortment,
    Landing,
}

impl Kind {
    pub(super) const fn count(self) -> usize {
        match self {
            Self::Landing => 1,
            _ => 2,
        }
    }
}

pub(super) fn database(kind: Kind, present: bool) -> Vec<u8> {
    let entity = TestEntity::new(1, "Source", "synthetic/source.xml");
    let entity = match kind {
        Kind::Manual => entity
            .port(TestPort::new("first"))
            .port(TestPort::new("second"))
            .loadout(TestLoadout::Manual(vec![
                TestLoadoutEntry::reference("first", 99),
                TestLoadoutEntry::reference("second", 99),
            ])),
        Kind::Default => entity
            .port(TestPort::new("first").default_item(99))
            .port(TestPort::new("second").default_item(99)),
        Kind::Assortment => entity.loadout(TestLoadout::RandomGenerated(vec![99, 99])),
        Kind::Landing => entity.vehicle("", "").landing_system(99),
    };
    let builder = p4kx_testkit::entity_schema(DataForgeBuilder::new()).record(entity.record());
    let builder = if present {
        builder.record(TestEntity::new(99, "Target", "synthetic/target.xml").record())
    } else {
        builder
    };
    builder.build()
}

pub(super) fn guid(value: &Value<'_>) -> Option<Guid> {
    match value {
        Value::Reference(link) => Some(link.guid()),
        Value::Error(ValueError::DanglingReference { guid }) => Some(*guid),
        _ => None,
    }
}

pub(super) fn values<'tree, 'source>(
    tree: &'tree RecordTree<'source>,
) -> Vec<&'tree Value<'source>> {
    let mut pending: Vec<_> = tree.root().fields().iter().map(Field::value).collect();
    let mut result = Vec::new();
    while let Some(value) = pending.pop() {
        result.push(value);
        match value {
            Value::Struct(structure) => pending.extend(structure.fields().iter().map(Field::value)),
            Value::Array(array) => pending.extend(array.iter()),
            _ => {}
        }
    }
    result
}

pub(super) fn belongs(
    tree: &RecordTree<'_>,
    parent: &StructValue<'_>,
    selected: &Value<'_>,
) -> bool {
    let actual_parent = ptr::eq(tree.root(), parent)
        || values(tree)
            .iter()
            .any(|value| matches!(value, Value::Struct(structure) if ptr::eq(structure, parent)));
    actual_parent
        && parent.fields().iter().any(|field| {
            ptr::eq(field.value(), selected)
                || match field.value() {
                    Value::Array(array) => array.iter().any(|item| ptr::eq(item, selected)),
                    _ => false,
                }
        })
}

#[derive(Default)]
pub(super) struct Recorder {
    pub(super) seen: Mutex<Vec<LoadoutReferenceSelection>>,
    pub(super) stop: bool,
}

struct Borrowed<'tree, 'source> {
    tree: &'tree RecordTree<'source>,
    recorder: &'tree Recorder,
}

impl LoadoutPathObserver for Borrowed<'_, '_> {
    fn record_path(
        &self,
        _: &LoadoutSelection,
        _: &StructValue<'_>,
        _: &Value<'_>,
    ) -> ControlFlow<()> {
        ControlFlow::Continue(())
    }

    fn record_name(&self, _: &LoadoutSelection, _: &RecordTree<'_>) -> ControlFlow<()> {
        ControlFlow::Continue(())
    }

    fn record_reference(
        &self,
        selection: &LoadoutReferenceSelection,
        parent: &StructValue<'_>,
        value: &Value<'_>,
    ) -> ControlFlow<()> {
        assert!(
            belongs(self.tree, parent, value),
            "each callback borrows the actual tree and nearest Struct or its actual Array element"
        );
        let actual = guid(value)
            .expect("the accepted original getter is a reference or a dangling-reference datum");
        assert_eq!(
            actual.to_string(),
            "00000000-0000-0000-0000-000000000063",
            "the independent original datum supplies the selected GUID"
        );
        self.recorder
            .seen
            .lock()
            .expect("the test log is not poisoned")
            .push(selection.clone());
        if self.recorder.stop {
            ControlFlow::Break(())
        } else {
            ControlFlow::Continue(())
        }
    }
}

impl LoadoutReadObserver for Recorder {
    fn observe(&self, _: LoadoutReadEvent<'_, '_>) -> ControlFlow<()> {
        ControlFlow::Continue(())
    }

    fn with_record_paths(
        &self,
        record: &Record<'_>,
        tree: &RecordTree<'_>,
        parse: &mut dyn FnMut(Option<&dyn LoadoutPathObserver>) -> ControlFlow<()>,
    ) -> ControlFlow<()> {
        assert!(
            tree.is_from(record),
            "the actual scoped original tree belongs to its physical Record"
        );
        parse(Some(&Borrowed {
            tree,
            recorder: self,
        }))
    }
}
