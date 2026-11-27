// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::json_tree::Node;

/// The name table used to interpret a CHF hash.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ChfNameKind {
    /// Head rig from the DNA gender hash.
    Rig,
    /// Character item port.
    Port,
    /// Material attachment.
    Attachment,
    /// Material library sub-material.
    SubMaterial,
    /// Public material parameter.
    Parameter,
}

/// The emitted role of a field participating in a CHF name lookup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ChfJsonNameUse {
    /// Original hash whose label was found.
    ResolvedHash,
    /// Original hash for which no label was found.
    UnresolvedHash,
    /// Label emitted alongside its original hash.
    Label,
}

/// One actual hash lookup or output label in the emitted JSON.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChfJsonName {
    pub(crate) kind: ChfNameKind,
    pub(crate) hash: u32,
    pub(crate) usage: ChfJsonNameUse,
}

impl ChfJsonName {
    /// Returns the name table used for this field.
    #[must_use]
    pub const fn kind(self) -> ChfNameKind {
        self.kind
    }

    /// Returns the original hash used for the lookup.
    #[must_use]
    pub const fn hash(self) -> u32 {
        self.hash
    }

    /// Returns whether this field is the hash or the emitted label.
    #[must_use]
    pub const fn usage(self) -> ChfJsonNameUse {
        self.usage
    }
}

/// An emitted CHF scalar or empty container and its exact JSON Pointer.
#[derive(Debug, Clone, Copy)]
pub struct ChfJsonField<'json> {
    pointer: &'json str,
    name: Option<ChfJsonName>,
}

impl<'json> ChfJsonField<'json> {
    /// Returns the exact pointer to the emitted value.
    #[must_use]
    pub const fn pointer(self) -> &'json str {
        self.pointer
    }

    /// Returns the actual name lookup attached to this value, when present.
    #[must_use]
    pub const fn name(self) -> Option<ChfJsonName> {
        self.name
    }
}

pub(crate) fn observe(node: &Node, observe: &mut impl FnMut(ChfJsonField<'_>)) {
    visit(node, &mut String::new(), observe);
}

fn visit(node: &Node, pointer: &mut String, observe: &mut impl FnMut(ChfJsonField<'_>)) {
    let depth = pointer.len();
    match node {
        Node::List(items) if !items.is_empty() => {
            for (index, item) in items.iter().enumerate() {
                pointer.push('/');
                pointer.push_str(&index.to_string());
                visit(item, pointer, observe);
                pointer.truncate(depth);
            }
        }
        Node::Object(fields) if !fields.is_empty() => {
            for (key, value) in fields {
                pointer.push('/');
                for character in key.chars() {
                    match character {
                        '~' => pointer.push_str("~0"),
                        '/' => pointer.push_str("~1"),
                        _ => pointer.push(character),
                    }
                }
                visit(value, pointer, observe);
                pointer.truncate(depth);
            }
        }
        datum => {
            let name = match datum {
                Node::ChfHash(name) | Node::ChfLabel(_, name) => Some(*name),
                _ => None,
            };
            observe(ChfJsonField { pointer, name });
        }
    }
}
