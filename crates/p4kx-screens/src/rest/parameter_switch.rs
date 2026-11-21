// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cmp::Ordering;

use p4kx_dataforge::Guid;

use super::Rest;
use super::graph::{FieldKind, Node, Operations};

/// Returns the parameter and effective case key for an unambiguous tag on the selected widget.
pub(crate) fn parameter_for_tag<'a>(
    graph: &'a Operations,
    (widget, field): (usize, &str),
    matches: impl Fn(Guid) -> bool,
) -> Option<(&'a str, f64)> {
    let mut bindings = graph.bindings.iter().filter(|binding| {
        binding.widget == widget && binding.kind == FieldKind::Text && binding.field == field
    });
    let binding = bindings.next()?;
    if bindings.next().is_some() {
        return None;
    }
    let Node::TagSwitch { pairs, input, .. } = graph.nodes.get(binding.input?)? else {
        return None;
    };
    let Node::Param {
        parameter,
        default: Rest::Integer(_),
    } = graph.nodes.get((*input)?)?
    else {
        return None;
    };
    let mut matching = pairs.iter().filter(|(_, tag)| tag.is_some_and(&matches));
    let (key, selected) = matching.next()?;
    if matching.next().is_some() {
        return None;
    }
    let (_, first) = pairs
        .iter()
        .find(|(other, _)| key.partial_cmp(other).is_some_and(Ordering::is_eq))?;
    (first == selected).then_some((parameter.as_str(), *key))
}
