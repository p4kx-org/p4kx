// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::cryxml::{SourceAttribute, SourceElement};

/// The source nodes that decide whether a class light field can be inherited.
#[derive(Debug)]
pub(crate) enum Override<'a> {
    Absent(SourceElement<'a>),
    Value(SourceAttribute<'a>),
    RepeatedNodes(Vec<SourceElement<'a>>),
    RepeatedAttributes(Vec<SourceAttribute<'a>>),
}

pub(crate) fn find<'a>(light: SourceElement<'a>, path: &[String]) -> Option<Override<'a>> {
    let (name, parents) = path.split_last()?;
    let mut parent = light;
    for step in parents {
        let mut matches = parent
            .elements()
            .filter(|node| node.namespace().is_none() && node.name() == step);
        let Some(first) = matches.next() else {
            return Some(Override::Absent(parent));
        };
        if let Some(second) = matches.next() {
            let mut repeated = vec![first, second];
            repeated.extend(matches);
            return Some(Override::RepeatedNodes(repeated));
        }
        parent = first;
    }
    let mut matches = parent
        .attributes()
        .filter(|attribute| attribute.namespace().is_none() && attribute.name() == name);
    match (matches.next(), matches.next()) {
        (None, _) => Some(Override::Absent(parent)),
        (Some(attribute), None) => Some(Override::Value(attribute)),
        (Some(first), Some(second)) => {
            let mut repeated = vec![first, second];
            repeated.extend(matches);
            Some(Override::RepeatedAttributes(repeated))
        }
    }
}
