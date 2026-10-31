// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The state a canvas has at rest: what its `operations` bind to `Instantiated`, `IsActive`,
//! `Alpha` and the parameters of nested canvases, evaluated with every game variable at its zero
//! value unless the caller sets it (ADR-0110, ADR-0259).

mod eval;
mod fields;
mod graph;
mod ops;
mod state;
#[cfg(test)]
mod tests;

use std::rc::Rc;

use p4kx_dataforge::Guid;

pub(crate) use eval::{Given, TEXT_FIELD};
pub(crate) use graph::Operations;
pub(crate) use state::{AtRest, key_of, same_parameter};

/// A value of the operations graph.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Rest {
    Bool(bool),
    Num(f64),
    Text(String),
    /// A tag, or none: what a tag node gives a state tag field or a string parameter.
    Tag(Option<Guid>),
}

/// The typed values a `WidgetCanvas` hands to the component parameters of its canvas.
pub(crate) type RestParams = Rc<[(String, Rest)]>;
