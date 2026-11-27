// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use serde::Serialize;

use super::ReducedError;
use super::cell::Cell;
use super::completeness;
use super::number::Number;
use crate::catalog_queries::AuthoredRestriction;
use crate::guid_text::CatalogGuid;
use crate::resource_number::ResourceNumber;
use crate::source_value_catalog::{SourceArenaOutput, SourceNode, SourcePayload, SourceText};

#[derive(Serialize)]
#[serde(untagged)]
pub(super) enum Restriction<'read, 'source, T> {
    Typed(T),
    Original(Original<'read, 'source>),
}

#[derive(Serialize)]
#[serde(untagged)]
pub(super) enum Original<'read, 'source> {
    Number(Number<'read>),
    Text(&'read SourceText<'source>),
    Label(&'source str),
    Boolean(bool),
    Guid(CatalogGuid),
}

pub(super) fn number<'read, 'source>(
    path: &'static str,
    value: &'read SourcedValue<AuthoredRestriction<'source, ResourceNumber>>,
) -> Result<Cell<Restriction<'read, 'source, Number<'read>>>, ReducedError> {
    project(path, value, Number::from)
}

pub(super) fn label<'read, 'source>(
    path: &'static str,
    value: &'read SourcedValue<AuthoredRestriction<'source, &'source str>>,
) -> Result<Cell<Restriction<'read, 'source, &'source str>>, ReducedError> {
    project(path, value, |label| *label)
}

fn project<'read, 'source, T, U>(
    path: &'static str,
    value: &'read SourcedValue<AuthoredRestriction<'source, T>>,
    typed: impl FnOnce(&'read T) -> U,
) -> Result<Cell<Restriction<'read, 'source, U>>, ReducedError> {
    completeness::cell(path, value)?.try_map(|value| match value {
        AuthoredRestriction::Typed(value) => Ok(Restriction::Typed(typed(value))),
        AuthoredRestriction::Original(arena) => {
            let root = selected_root(path, arena)?;
            let payload = completeness::cell(path, root.value())?
                .value
                .ok_or(ReducedError::IncompleteScope(path))?;
            scalar(path, payload).map(Restriction::Original)
        }
    })
}

pub(super) fn subtypes<'read, 'source>(
    path: &'static str,
    value: &'read SourcedValue<SourceArenaOutput<'source>>,
) -> Result<Cell<Vec<Cell<Original<'read, 'source>>>>, ReducedError> {
    completeness::cell(path, value)?.try_map(|arena| read_subtypes(path, arena))
}

fn read_subtypes<'read, 'source>(
    path: &'static str,
    arena: &'read SourceArenaOutput<'source>,
) -> Result<Vec<Cell<Original<'read, 'source>>>, ReducedError> {
    let root = selected_root(path, arena)?;
    let Some(SourcePayload::Array { elements }) = completeness::cell(path, root.value())?.value
    else {
        return Err(ReducedError::IncompleteScope(path));
    };
    elements
        .iter()
        .map(|element| {
            let node = arena
                .value(*element)
                .ok_or(ReducedError::IncompleteScope(path))?;
            completeness::cell(path, node.value())?.try_map(|payload| match payload {
                SourcePayload::String { .. }
                | SourcePayload::Locale { .. }
                | SourcePayload::Enum { .. } => scalar(path, payload),
                _ => Err(ReducedError::IncompleteScope(path)),
            })
        })
        .collect()
}

fn selected_root<'read, 'source>(
    path: &'static str,
    arena: &'read SourceArenaOutput<'source>,
) -> Result<&'read SourceNode<'source>, ReducedError> {
    let [field] = arena.roots() else {
        return Err(ReducedError::IncompleteScope(path));
    };
    arena
        .value(field.value())
        .ok_or(ReducedError::IncompleteScope(path))
}

fn scalar<'read, 'source>(
    path: &'static str,
    value: &'read SourcePayload<'source>,
) -> Result<Original<'read, 'source>, ReducedError> {
    match value {
        SourcePayload::String { value } | SourcePayload::Locale { value } => {
            Ok(Original::Text(value))
        }
        SourcePayload::Enum { value } => Ok(Original::Label(value)),
        SourcePayload::Number { value } => Ok(Original::Number(value.into())),
        SourcePayload::Boolean { value } => Ok(Original::Boolean(*value)),
        SourcePayload::Guid { value } => Ok(Original::Guid(*value)),
        SourcePayload::Array { .. }
        | SourcePayload::Struct { .. }
        | SourcePayload::RecordReference { .. }
        | SourcePayload::WeakReference { .. } => Err(ReducedError::IncompleteScope(path)),
    }
}
