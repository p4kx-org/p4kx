// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::convert::Infallible;
use std::fmt::{self, Debug, Display, Formatter, Write};
use std::iter;
use std::sync::{Arc, OnceLock};

use crate::Guid;
use crate::tags::TAG_PATH_SEPARATOR;

#[derive(Debug)]
pub(crate) struct Node {
    pub(crate) guid: Guid,
    pub(crate) name: Arc<str>,
    pub(crate) path_bytes: u64,
    pub(crate) joined: OnceLock<Box<str>>,
    pub(crate) legacy_id: Option<u32>,
    pub(crate) parent: Option<u32>,
    pub(crate) children: Vec<u32>,
    pub(crate) end: u32,
    pub(crate) depth: u32,
    pub(crate) listed: bool,
}

/// A borrowed path through the names of a selected tag and its ancestors.
#[derive(Clone, Copy)]
pub struct TagPath<'tree> {
    nodes: &'tree [Node],
    leaf: Option<&'tree Node>,
}

impl<'tree> TagPath<'tree> {
    pub(crate) const fn new(nodes: &'tree [Node], leaf: Option<&'tree Node>) -> Self {
        Self { nodes, leaf }
    }

    /// Returns the UTF-8 length of the joined path in bytes.
    #[must_use]
    pub fn byte_len(&self) -> u64 {
        self.leaf.map_or(0, |node| node.path_bytes)
    }

    /// Returns the joined path and retains its text in the tree.
    #[must_use]
    pub fn as_str(&self) -> &'tree str {
        let Some(node) = self.leaf else {
            return "";
        };
        if node.parent.is_none() {
            return &node.name;
        }
        node.joined
            .get_or_init(|| self.to_string().into_boxed_str())
    }

    /// Compares the joined path with text, ignoring ASCII case.
    #[must_use]
    pub fn eq_ignore_ascii_case(&self, text: &str) -> bool {
        self.matches(text, <[u8]>::eq_ignore_ascii_case)
    }

    /// Compares the joined path with text using exact spelling and case.
    #[must_use]
    pub fn eq_text(&self, text: &str) -> bool {
        self.matches(text, PartialEq::eq)
    }

    fn matches(&self, text: &str, equal: impl Fn(&[u8], &[u8]) -> bool) -> bool {
        if self.byte_len() != text.len() as u64 {
            return false;
        }
        let mut remaining = text.as_bytes();
        let mut encoded = [0; 4];
        let separator = TAG_PATH_SEPARATOR.encode_utf8(&mut encoded).as_bytes();
        for node in self.lineage() {
            let name = node.name.as_bytes();
            let Some(prefix_len) = remaining.len().checked_sub(name.len()) else {
                return false;
            };
            let (prefix, suffix) = remaining.split_at(prefix_len);
            if !equal(suffix, name) {
                return false;
            }
            if node.parent.is_none() {
                return prefix.is_empty();
            }
            let Some(prefix) = prefix.strip_suffix(separator) else {
                return false;
            };
            remaining = prefix;
        }
        remaining.is_empty()
    }

    /// Appends the joined path without retaining a copy in the tree.
    pub fn append_to(&self, destination: &mut String) {
        match self.render(|part| {
            destination.push_str(part);
            Ok::<(), Infallible>(())
        }) {
            Ok(()) => {}
            Err(never) => match never {},
        }
    }

    /// Writes the joined path without retaining a copy in the tree.
    ///
    /// # Errors
    ///
    /// Returns a formatting error from the destination.
    pub fn write_to(&self, destination: &mut impl Write) -> fmt::Result {
        self.render(|part| destination.write_str(part))
    }

    fn render<E>(&self, mut write: impl FnMut(&str) -> Result<(), E>) -> Result<(), E> {
        let mut lineage: Vec<_> = self.lineage().map(|node| node.name.as_ref()).collect();
        if let Some(root) = lineage.pop() {
            write(root)?;
        }
        let mut encoded = [0; 4];
        let separator = TAG_PATH_SEPARATOR.encode_utf8(&mut encoded);
        while let Some(name) = lineage.pop() {
            write(separator)?;
            write(name)?;
        }
        Ok(())
    }

    fn lineage(&self) -> impl Iterator<Item = &'tree Node> {
        let nodes = self.nodes;
        iter::successors(self.leaf, move |node| nodes.get(node.parent? as usize))
    }
}

impl Display for TagPath<'_> {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        self.write_to(formatter)
    }
}

impl Debug for TagPath<'_> {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TagPath")
            .field("bytes", &self.byte_len())
            .finish()
    }
}
