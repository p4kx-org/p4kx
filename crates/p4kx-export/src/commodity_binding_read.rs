// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    RecordTree, SourceUse, SourceUseRole, SourceValue, SourceValueState, SourcedValue, StructValue,
    Value,
};

use crate::CatalogError;
use crate::catalog_origins::CatalogOrigins;
use crate::commodity_authoring_inputs;
use crate::commodity_inputs::CommodityInputs;
use crate::commodity_text_input::CommodityTextInput;
use crate::resource_value_read::ResourceFields;
use crate::source_fields::FieldError;

/// One classified owning array, shared by its independently typed binding families.
pub(crate) struct BindingArray<'tree, 'source> {
    name: &'static str,
    value: SourcedValue<()>,
    entries: Vec<Entry<'tree, 'source>>,
}

struct Entry<'tree, 'source> {
    position: usize,
    source: Option<&'tree StructValue<'source>>,
    state: SourceValueState,
}

impl<'tree, 'source> BindingArray<'tree, 'source> {
    pub(crate) fn new(
        tree: &'tree RecordTree<'source>,
        origins: &CatalogOrigins,
        name: &'static str,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let context = origins.resource(tree.record_index());
        let value = ResourceFields::new(tree.root(), "", &context)
            .array(name, notes)?
            .map(|_| ());
        let mut entries = Vec::new();
        if let Some(Value::Array(values)) = tree.field(name) {
            for (position, raw) in values.iter().enumerate() {
                let path = format!("{name}[{position}]");
                let source = context.structure_element(raw, tree.type_name(), &path, notes)?;
                entries.push(Entry {
                    position,
                    source: source.value().copied(),
                    state: source.state(),
                });
            }
        }
        Ok(Self {
            name,
            value,
            entries,
        })
    }

    /// Returns only the selectors used before the first proved component match.
    pub(crate) fn first_type_inputs(
        &self,
        record_index: u32,
        origins: &CatalogOrigins,
        wanted: &str,
    ) -> Result<(Vec<SourceUse>, bool), CatalogError> {
        let mut inputs = CommodityInputs::new(origins);
        let complete = matches!(
            self.value.state(),
            SourceValueState::Present | SourceValueState::Absent | SourceValueState::Null
        );
        let role = if complete {
            SourceUseRole::Selector
        } else {
            SourceUseRole::Attempted
        };
        inputs.at(record_index, self.name, role)?;
        inputs.require_complete(complete);
        for entry in &self.entries {
            let path = format!("{}[{}]", self.name, entry.position);
            let path = if entry.source.is_some() {
                format!("{path}.$type")
            } else {
                path
            };
            let complete = matches!(
                entry.state,
                SourceValueState::Present | SourceValueState::Null
            );
            let role = if complete {
                SourceUseRole::Selector
            } else {
                SourceUseRole::Attempted
            };
            inputs.at(record_index, &path, role)?;
            inputs.require_complete(complete);
            if entry
                .source
                .is_some_and(|source| source.type_name() == wanted)
            {
                break;
            }
        }
        Ok(inputs.into_evidence())
    }
}

/// The original array position and concrete component selector.
pub(crate) struct BindingAt<'path> {
    pub(crate) path: &'path str,
    pub(crate) position: usize,
}

/// Retained typed data and the actual handles required by its aggregate.
pub(crate) struct BindingFacts<T> {
    pub(crate) value: T,
    pub(crate) inputs: Vec<SourceUse>,
    pub(crate) complete: bool,
}

impl<T> BindingFacts<T> {
    pub(crate) fn new(value: T, inputs: CommodityInputs<'_>) -> Self {
        let (inputs, complete) = inputs.into_evidence();
        Self {
            value,
            inputs,
            complete,
        }
    }
}

pub(crate) struct BindingRead<'source, T, I> {
    pub(crate) value: SourcedValue<Vec<SourcedValue<T>>>,
    pub(crate) first: Option<I>,
    pub(crate) parent: CommodityTextInput<'source>,
    pub(crate) inputs: Vec<SourceUse>,
    pub(crate) complete: bool,
}

/// Collects proved matching bindings with precise child and selection contributions.
pub(crate) fn collect<'source, T, I>(
    tree: &RecordTree<'source>,
    origins: &CatalogOrigins,
    array: &BindingArray<'_, 'source>,
    wanted: &str,
    notes: &mut Vec<FieldError>,
    mut read: impl FnMut(
        &StructValue<'source>,
        &BindingAt<'_>,
        &mut Vec<FieldError>,
    ) -> Result<(BindingFacts<T>, I), CatalogError>,
) -> Result<BindingRead<'source, T, I>, CatalogError> {
    let context = origins.resource(tree.record_index());
    let state = array.value.state();
    let mut selection = Selection::new(array.name, state);
    let mut inputs = CommodityInputs::new(origins);
    array_selector(&mut inputs, tree.record_index(), array.name, state)?;
    let mut rows = Vec::new();
    let mut first = None;
    for entry in &array.entries {
        let path = format!("{}[{}]", array.name, entry.position);
        let selector = if entry.source.is_some() {
            format!("{path}.$type")
        } else {
            path.clone()
        };
        selection.observe(&selector, entry.state, first.is_none());
        array_selector(&mut inputs, tree.record_index(), &selector, entry.state)?;
        let Some(source) = entry.source.filter(|source| source.type_name() == wanted) else {
            continue;
        };
        let at = BindingAt {
            path: &path,
            position: entry.position,
        };
        let (facts, input) = read(source, &at, notes)?;
        let mut element = CommodityInputs::new(origins);
        element.extend(facts.inputs, facts.complete);
        rows.push(element.value(facts.value));
        let (used, complete) = element.into_evidence();
        inputs.extend(used, complete);
        if first.is_none() {
            first = Some(input);
        }
    }
    inputs.require_complete(selection.complete);
    let (inputs, complete) = inputs.into_evidence();
    let value =
        commodity_authoring_inputs::without_payload(state).unwrap_or(SourceValue::Present(rows));
    let value = origins.uses.value(value, &inputs, complete);
    let parent = CommodityTextInput::parent(
        &context,
        tree.record_index(),
        array.name,
        state,
        selection.first_paths,
        selection.first_complete,
    )?;
    Ok(BindingRead {
        value,
        first,
        parent,
        inputs,
        complete,
    })
}

fn array_selector(
    inputs: &mut CommodityInputs<'_>,
    record_index: u32,
    path: &str,
    state: SourceValueState,
) -> Result<(), CatalogError> {
    let role = if state == SourceValueState::Unreadable {
        SourceUseRole::Attempted
    } else {
        SourceUseRole::Selector
    };
    inputs.at(record_index, path, role)
}

struct Selection {
    first_paths: Vec<String>,
    complete: bool,
    first_complete: bool,
}

impl Selection {
    fn new(array: &str, state: SourceValueState) -> Self {
        let complete = matches!(
            state,
            SourceValueState::Present | SourceValueState::Absent | SourceValueState::Null
        );
        let first_paths = if complete {
            vec![array.to_owned()]
        } else {
            Vec::new()
        };
        Self {
            first_paths,
            complete,
            first_complete: complete,
        }
    }

    fn observe(&mut self, path: &str, state: SourceValueState, before_first: bool) {
        let complete = matches!(state, SourceValueState::Present | SourceValueState::Null);
        if before_first && state != SourceValueState::Unreadable {
            self.first_paths.push(path.to_owned());
        }
        self.complete &= complete;
        if before_first {
            self.first_complete &= complete;
        }
    }
}
