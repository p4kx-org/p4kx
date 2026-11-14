// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;

use p4kx_extraction::CancelToken;

use crate::CatalogError;

#[path = "mission_title_graph.rs"]
mod graph;
#[path = "mission_title_key.rs"]
mod key;
#[path = "mission_title_plan.rs"]
mod plan;
#[path = "mission_title_render.rs"]
mod render;

pub(crate) use key::AsciiKey;

type Resolve<'query, 'source> = dyn FnMut(&'source str, &str, Option<&str>) -> Result<Option<Token<'source>>, CatalogError>
    + 'query;
type Localise<'query, 'source> =
    dyn FnMut(&'source str) -> Result<Option<Cow<'source, str>>, CatalogError> + 'query;

/// What a mission token denotes in the selected property layer.
pub(crate) enum Token<'source> {
    /// The selected locale key for this property.
    Key(&'source str),
    /// A runtime value displayed with the authored bracket convention.
    Placeholder(&'source str),
}

/// Expands one actual language with fallible source lookup and token selection.
pub(crate) fn render_language<'source>(
    key: &'source str,
    text: Cow<'source, str>,
    localise: &mut Localise<'_, 'source>,
    resolve: &mut Resolve<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<Cow<'source, str>, CatalogError> {
    if !text.contains(graph::OPEN) {
        return Ok(text);
    }
    render_key(key, text, localise, resolve, cancel).map(Cow::Owned)
}

fn render_key<'source>(
    key: &'source str,
    text: Cow<'source, str>,
    localise: &mut Localise<'_, 'source>,
    resolve: &mut Resolve<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<String, CatalogError> {
    let graph = graph::Graph::build(key, text, localise, resolve, cancel)?;
    let plan = plan::analyze(&graph, cancel)?;
    render::write(&graph, &plan, cancel)
}

pub(super) fn check_cancel(cancel: Option<&CancelToken>) -> Result<(), CatalogError> {
    if cancel.is_some_and(CancelToken::is_cancelled) {
        return Err(CatalogError::Cancelled);
    }
    Ok(())
}

fn push<T>(items: &mut Vec<T>, value: T) -> Result<(), CatalogError> {
    items
        .try_reserve(1)
        .map_err(CatalogError::MissionTextAllocation)?;
    items.push(value);
    Ok(())
}

#[cfg(test)]
#[path = "mission_title_tests.rs"]
mod tests;
