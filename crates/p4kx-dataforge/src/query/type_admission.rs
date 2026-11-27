// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use super::schema::{AncestorStep, StructFields, ancestors_include_observed};
use super::suggest::closest;
use super::{Name, QueryError, QueryErrorKind};

/// An original Struct declaration consulted while admitting types for FROM.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum QueryTypeSource {
    /// A physical type name compared with the requested name.
    #[non_exhaustive]
    Name {
        /// Original Struct definition position.
        index: usize,
        /// The actual original name bytes, absent when the stored name could not be read.
        span: Option<Range<u64>>,
    },
    /// A physical parent word consulted while deciding one candidate's ancestry.
    #[non_exhaustive]
    Parent {
        /// The original candidate Struct index.
        candidate: usize,
        /// The original Struct containing the parent word.
        index: usize,
        /// The original stored word, including the no-parent sentinel.
        stored_parent: u32,
    },
    /// An ancestry check ended without a complete original chain.
    #[non_exhaustive]
    UnreadableAncestry {
        /// The original candidate Struct index.
        candidate: usize,
        /// The unresolved table position.
        index: usize,
    },
}

pub(super) fn select_type(
    structs: &StructFields<'_, '_>,
    name: &Name,
    mut observe: Option<&mut dyn FnMut(QueryTypeSource)>,
) -> Result<Vec<bool>, QueryError> {
    let targets = (0..structs.len())
        .filter(|&index| {
            let original = structs.name(index);
            if let Some(observe) = observe.as_deref_mut() {
                observe(QueryTypeSource::Name {
                    index,
                    span: original.and_then(|name| structs.forge().original_text_span(name)),
                });
            }
            original == Some(name.as_str())
        })
        .collect::<Vec<_>>();
    if targets.is_empty() {
        return Err(QueryError::new(
            QueryErrorKind::UnknownType {
                name: name.as_str().to_owned(),
                suggestion: closest(name.as_str(), structs.names()),
            },
            name.span(),
        ));
    }
    let selected = (0..structs.len())
        .map(|candidate| {
            ancestors_include_observed(structs.forge(), candidate, &targets, |step| {
                if let Some(observe) = observe.as_deref_mut() {
                    observe(parent_source(candidate, step));
                }
            })
        })
        .collect();
    Ok(selected)
}

const fn parent_source(candidate: usize, step: AncestorStep) -> QueryTypeSource {
    match step {
        AncestorStep::Parent {
            index,
            stored_parent,
        } => QueryTypeSource::Parent {
            candidate,
            index,
            stored_parent,
        },
        AncestorStep::Unavailable { index } => {
            QueryTypeSource::UnreadableAncestry { candidate, index }
        }
    }
}
