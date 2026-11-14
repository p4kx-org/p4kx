// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::ops::ControlFlow;
use std::sync::Mutex;

use p4kx_assembly::{
    LoadoutError, LoadoutFileObservation, LoadoutPath, LoadoutPathObserver, LoadoutPathOperation,
    LoadoutReadEvent, LoadoutReadObserver, LoadoutRequest, LoadoutSelection, LoadoutXmlDecoding,
    XmlLoadoutPathSite,
};
use p4kx_dataforge::{Record, RecordTree, StructValue, Value};

pub(crate) type TestResult = Result<(), Box<dyn std::error::Error>>;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum EventKind {
    Read,
    Projection,
    Dependency,
    Reused,
}

pub(crate) struct Event {
    pub(crate) kind: EventKind,
    pub(crate) observation: LoadoutFileObservation,
    pub(crate) request: LoadoutPath,
    pub(crate) dependency: Option<LoadoutRequest>,
    pub(crate) parent: Option<LoadoutFileObservation>,
    pub(crate) caller: Option<LoadoutRequest>,
}

pub(crate) struct Refusal {
    pub(crate) parent: LoadoutFileObservation,
    pub(crate) dependency: LoadoutRequest,
    pub(crate) error: LoadoutError,
}

pub(crate) struct XmlSite {
    pub(crate) selection: LoadoutSelection,
    pub(crate) observation: LoadoutFileObservation,
    pub(crate) node: u32,
    pub(crate) name: String,
    pub(crate) value: String,
    pub(crate) decoding: LoadoutXmlDecoding,
}

#[derive(Default)]
pub(crate) struct State {
    pub(crate) selections: HashMap<LoadoutSelection, Option<u32>>,
    pub(crate) events: Vec<Event>,
    pub(crate) refusals: Vec<Refusal>,
    pub(crate) xml: Vec<XmlSite>,
    pub(crate) derived: Vec<LoadoutPathOperation>,
}

#[derive(Default)]
pub(crate) struct Probe(pub(crate) Mutex<State>);

struct Scoped<'probe, 'tree, 'source> {
    probe: &'probe Probe,
    tree: &'tree RecordTree<'source>,
}

impl LoadoutPathObserver for Scoped<'_, '_, '_> {
    fn record_path(
        &self,
        selection: &LoadoutSelection,
        parent: &StructValue<'_>,
        value: &Value<'_>,
    ) -> ControlFlow<()> {
        assert!(
            parent
                .fields()
                .iter()
                .any(|field| std::ptr::eq(field.value(), value)),
            "the actual winning Value belongs to its actual borrowed parent"
        );
        let Ok(mut state) = self.probe.0.lock() else {
            return ControlFlow::Break(());
        };
        state
            .selections
            .insert(selection.clone(), Some(self.tree.record_index()));
        ControlFlow::Continue(())
    }

    fn record_name(&self, selection: &LoadoutSelection, tree: &RecordTree<'_>) -> ControlFlow<()> {
        assert!(
            std::ptr::eq(self.tree, tree),
            "the class fallback borrows the same actual tree"
        );
        let Ok(mut state) = self.probe.0.lock() else {
            return ControlFlow::Break(());
        };
        state
            .selections
            .insert(selection.clone(), Some(tree.record_index()));
        ControlFlow::Continue(())
    }
}

impl LoadoutReadObserver for Probe {
    fn observe(&self, event: LoadoutReadEvent<'_, '_>) -> ControlFlow<()> {
        let Ok(mut state) = self.0.lock() else {
            return ControlFlow::Break(());
        };
        if let LoadoutReadEvent::FileDependencyRefused {
            parent,
            dependency,
            error,
        } = event
        {
            state.refusals.push(Refusal {
                parent: parent.clone(),
                dependency: dependency.clone(),
                error: error.clone(),
            });
            return ControlFlow::Continue(());
        }
        let Some(event) = file_event(&event) else {
            return ControlFlow::Continue(());
        };
        assert!(
            event
                .request
                .selection()
                .is_some_and(|selection| state.selections.contains_key(selection)),
            "each actual document candidate carries a captured authentic selection"
        );
        state.events.push(event);
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
            "the borrowed canonical tree belongs to its actual Record"
        );
        parse(Some(&Scoped { probe: self, tree }))
    }

    fn xml_path(
        &self,
        selection: &LoadoutSelection,
        site: &XmlLoadoutPathSite<'_>,
    ) -> ControlFlow<()> {
        let Ok(mut state) = self.0.lock() else {
            return ControlFlow::Break(());
        };
        state.selections.insert(selection.clone(), None);
        state.xml.push(XmlSite {
            selection: selection.clone(),
            observation: site.observation().clone(),
            node: site.node_index(),
            name: site.attribute_name().to_owned(),
            value: site.value().to_owned(),
            decoding: site.decoding(),
        });
        ControlFlow::Continue(())
    }

    fn derived_path(
        &self,
        selection: &LoadoutSelection,
        inputs: &[&LoadoutPath],
        _value: &str,
    ) -> ControlFlow<()> {
        let Ok(mut state) = self.0.lock() else {
            return ControlFlow::Break(());
        };
        assert!(
            inputs.iter().all(|path| path
                .selection()
                .is_some_and(|source| state.selections.contains_key(source))),
            "vehicle candidates derive only from actual private source pairs"
        );
        state.selections.insert(selection.clone(), None);
        state.derived.push(selection.operation());
        ControlFlow::Continue(())
    }
}

pub(crate) const fn caller_path(caller: &LoadoutRequest) -> Option<&LoadoutPath> {
    match caller {
        LoadoutRequest::Xml { path } => Some(path),
        LoadoutRequest::Vehicle { definition, .. } => Some(definition),
        _ => None,
    }
}

pub(crate) fn assert_current_caller(state: &State, event: &Event, source: u32) -> TestResult {
    let selection = event
        .caller
        .as_ref()
        .and_then(caller_path)
        .and_then(LoadoutPath::selection)
        .ok_or("the genuine current caller is absent")?;
    assert_eq!(
        state.selections.get(selection),
        Some(&Some(source)),
        "cache reuse carries the current getter's original physical Record"
    );
    Ok(())
}

fn file_event(event: &LoadoutReadEvent<'_, '_>) -> Option<Event> {
    let (kind, parent, observation, request, dependency, caller) = match *event {
        LoadoutReadEvent::FileRead {
            observation,
            request,
            caller,
        } => (
            EventKind::Read,
            None,
            observation,
            request,
            None,
            Some(caller),
        ),
        LoadoutReadEvent::FileProjection {
            observation,
            request,
            caller,
        } => (
            EventKind::Projection,
            None,
            observation,
            request,
            None,
            Some(caller),
        ),
        LoadoutReadEvent::FileDependency {
            parent,
            observation,
            request,
            dependency,
        } => (
            EventKind::Dependency,
            Some(parent),
            observation,
            request,
            Some(dependency),
            None,
        ),
        LoadoutReadEvent::FileReused {
            observation,
            request,
            caller,
        } => (
            EventKind::Reused,
            None,
            observation,
            request,
            None,
            Some(caller),
        ),
        _ => return None,
    };
    Some(Event {
        kind,
        parent: parent.cloned(),
        observation: observation.clone(),
        request: request.clone(),
        dependency: dependency.cloned(),
        caller: caller.cloned(),
    })
}
