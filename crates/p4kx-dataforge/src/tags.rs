// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;

use crate::tag_build::layout;
use crate::{DataForge, Guid, TagResolver};

#[path = "tag_events.rs"]
mod events;
#[path = "tag_path.rs"]
mod path;

pub(crate) use path::Node;
pub use path::TagPath;

pub use events::{TagBuildEvent, TagPathEdgeKind, TagPathIncoming, TagRelationIssue};

/// The struct of the Record that lists the root tags.
pub const TAG_DATABASE_TYPE: &str = "TagDatabase";

/// The character that joins the names of a tag and its ancestors into the tag path.
pub const TAG_PATH_SEPARATOR: char = '.';

/// Historical tag depth limit.
#[deprecated(note = "Tag trees retain the full readable hierarchy")]
pub const MAX_TAG_DEPTH: usize = 64;

/// Historical tag name byte limit.
#[deprecated(note = "Tag trees retain the full readable name")]
pub const MAX_TAG_NAME_BYTES: usize = 256;

/// Historical joined tag path byte limit.
#[deprecated(note = "Tag trees retain the full selected path")]
pub const MAX_TAG_PATH_BYTES: usize = 1024;

/// Historical cumulative tag path byte limit.
#[deprecated(note = "Tag trees retain all readable selected paths")]
pub const MAX_TAG_TEXT_BYTES: usize = 64 << 20;

/// What the tag tree could not take from the DataForge. The tree is built from whatever reads.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TagTreeIssues {
    pub(crate) unclassified: usize,
    pub(crate) unreadable: usize,
    pub(crate) dangling: usize,
    pub(crate) repeated: usize,
    pub(crate) duplicate: usize,
    pub(crate) unlisted: usize,
}

impl TagTreeIssues {
    /// Returns the number of Records whose unreadable type prevents classifying tag-family membership.
    #[must_use]
    pub const fn unclassified(&self) -> usize {
        self.unclassified
    }

    /// Returns the number of Tag Records whose fields cannot be read; they are left out.
    #[must_use]
    pub const fn unreadable(&self) -> usize {
        self.unreadable
    }

    /// Returns the number of listed children and roots that are no Tag Record.
    #[must_use]
    pub const fn dangling(&self) -> usize {
        self.dangling
    }

    /// Returns the number of times a tag was listed again, as a child of another tag or as a
    /// root; the first listing wins, which also cuts a cycle.
    #[must_use]
    pub const fn repeated(&self) -> usize {
        self.repeated
    }

    /// Returns zero because readable depth does not omit tags.
    #[deprecated(note = "Tag trees retain the full readable hierarchy")]
    #[must_use]
    pub const fn too_deep(&self) -> usize {
        0
    }

    /// Returns zero because readable names and paths do not omit tags.
    #[deprecated(note = "Tag trees retain the full readable name and path")]
    #[must_use]
    pub const fn too_long(&self) -> usize {
        0
    }

    /// Returns the number of Tag Records left out because an earlier one has their GUID.
    #[must_use]
    pub const fn duplicate(&self) -> usize {
        self.duplicate
    }

    /// Returns the number of tags the `TagDatabase` does not reach; they are roots of their own
    /// or below one.
    #[must_use]
    pub const fn unlisted(&self) -> usize {
        self.unlisted
    }

    /// Returns whether the tree is exactly what the DataForge lists.
    #[must_use]
    pub const fn is_clean(&self) -> bool {
        self.unclassified == 0
            && self.unreadable == 0
            && self.dangling == 0
            && self.repeated == 0
            && self.duplicate == 0
            && self.unlisted == 0
    }
}

/// The tag tree of a DataForge: the `TagDatabase` with its tags and their children, each tag with
/// its tag path.
///
/// Many fields (`Tags`, `RequiredTags`, `PortTags`, …) refer to a Tag Record by GUID. The tree
/// answers which tag a GUID is, where it sits and what is below it. It borrows nothing from the
/// DataForge it was built from. Tags are stored in pre-order, so the tags below one tag follow it
/// directly.
#[derive(Debug, Default)]
pub struct TagTree {
    nodes: Vec<Node>,
    roots: Vec<u32>,
    by_guid: Box<[(Guid, u32)]>,
    issues: TagTreeIssues,
}

impl TagTree {
    /// Builds the tag tree of `forge`: every `TagDatabase` Record lists the root tags, and every
    /// Tag Record its children.
    ///
    /// A Tag Record that is not below the `TagDatabase` becomes a root of its own, unlisted, so
    /// every tag a field refers to has a path. A tag listed twice stays where it was listed first.
    /// The tree of a DataForge without tags is empty. What cannot be read is counted in
    /// [`TagTree::issues`].
    #[must_use]
    pub fn build(forge: &DataForge<'_>) -> Self {
        Self::build_with(forge, &|| false).unwrap_or_default()
    }

    /// Builds the tag tree like [`build`](Self::build), and gives up with `None` once `stop`
    /// returns `true`; `stop` is polled between Records and between tags.
    #[must_use]
    pub fn build_with(forge: &DataForge<'_>, stop: &dyn Fn() -> bool) -> Option<Self> {
        layout(forge, stop)
            .map(Self::from_layout)
            .filter(|_| !stop())
    }

    /// Builds a tag tree and reports actual source, relation, and projection operations.
    ///
    /// Returns `None` once `stop` requests cancellation.
    ///
    /// # Errors
    ///
    /// Returns the observer's error without completing the tree.
    pub fn build_observed<'f, E>(
        forge: &'f DataForge<'f>,
        stop: &dyn Fn() -> bool,
        observe: &mut impl for<'r> FnMut(TagBuildEvent<'r, 'f>) -> Result<(), E>,
    ) -> Result<Option<Self>, E> {
        let Some(layout) = crate::tag_build::layout_observed(forge, stop, observe)? else {
            return Ok(None);
        };
        let tree = Self::from_layout(layout);
        if stop() {
            observe(TagBuildEvent::Cancelled)?;
            return Ok(None);
        }
        Ok(Some(tree))
    }

    fn from_layout(laid: crate::tag_build::Layout) -> Self {
        let mut by_guid: Vec<(Guid, u32)> = (0..)
            .zip(&laid.nodes)
            .map(|(index, node)| (node.guid, index))
            .collect();
        by_guid.sort_unstable();
        Self {
            nodes: laid.nodes,
            roots: laid.roots,
            by_guid: by_guid.into_boxed_slice(),
            issues: laid.issues,
        }
    }

    /// Returns the number of tags in the tree.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Returns whether the tree has no tag.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Returns what could not be taken from the DataForge.
    #[must_use]
    pub const fn issues(&self) -> TagTreeIssues {
        self.issues
    }

    /// Returns the root tags in the order the `TagDatabase` lists them, then the unlisted ones.
    pub fn roots(&self) -> impl ExactSizeIterator<Item = Tag<'_>> {
        self.roots.iter().map(|&index| Tag { tree: self, index })
    }

    /// Returns every tag in pre-order: a tag, then everything below it, then its next sibling.
    pub fn iter(&self) -> impl ExactSizeIterator<Item = Tag<'_>> {
        (0..u32::try_from(self.nodes.len()).unwrap_or(u32::MAX))
            .map(|index| Tag { tree: self, index })
    }

    /// Returns the tag at `position` in pre-order, as [`Tag::position`] counts, or `None` past the
    /// last tag.
    #[must_use]
    pub fn at(&self, position: usize) -> Option<Tag<'_>> {
        let index = u32::try_from(position).ok()?;
        self.node(index)?;
        Some(Tag { tree: self, index })
    }

    /// Returns the tag with the Tag Record `guid`, or `None` when the tree has none.
    #[must_use]
    pub fn get(&self, guid: Guid) -> Option<Tag<'_>> {
        let first = self.by_guid.partition_point(|(other, _)| *other < guid);
        let &(found, index) = self.by_guid.get(first)?;
        (found == guid).then_some(Tag { tree: self, index })
    }

    /// Returns every tag whose tag path is `path`, ignoring ASCII case, in pre-order. A tag path is
    /// not unique: sibling tags may share a name, so the GUID stays the identity.
    pub fn find_path<'t, 'p>(
        &'t self,
        path: &'p str,
    ) -> impl Iterator<Item = Tag<'t>> + use<'t, 'p> {
        self.iter()
            .filter(move |tag| tag.path_view().eq_ignore_ascii_case(path))
    }

    /// Returns the tag path of the Tag Record `guid`, e.g. `Weapon.Gun`, or `None` when the tree
    /// has no such tag.
    #[must_use]
    pub fn path_of(&self, guid: Guid) -> Option<&str> {
        self.get(guid).map(|tag| tag.path())
    }

    fn node(&self, index: u32) -> Option<&Node> {
        self.nodes.get(usize::try_from(index).ok()?)
    }
}

impl TagResolver for TagTree {
    fn tag_path(&self, guid: Guid) -> Option<&str> {
        self.path_of(guid)
    }

    fn tag_path_text(&self, guid: Guid) -> Option<Cow<'_, str>> {
        self.get(guid)
            .map(|tag| Cow::Owned(tag.path_view().to_string()))
    }
}

/// One tag of a [`TagTree`].
#[derive(Debug, Clone, Copy)]
pub struct Tag<'t> {
    tree: &'t TagTree,
    index: u32,
}

impl<'t> Tag<'t> {
    fn node(&self) -> Option<&'t Node> {
        self.tree.node(self.index)
    }

    /// Returns the place of the tag in the pre-order of [`TagTree::iter`]; the tags below it
    /// follow at the next places, as many as [`Tag::descendants`] has.
    #[must_use]
    pub const fn position(&self) -> usize {
        self.index as usize
    }

    /// Returns the GUID of the Tag Record, which fields refer to.
    #[must_use]
    pub fn guid(&self) -> Guid {
        self.node().map(|node| node.guid).unwrap_or_default()
    }

    /// Returns the name of the tag alone, e.g. `Gun`.
    #[must_use]
    pub fn name(&self) -> &'t str {
        self.node().map_or("", |node| &node.name)
    }

    /// Returns the tag path: the names from the root down to the tag, joined by
    /// [`TAG_PATH_SEPARATOR`], e.g. `Weapon.Gun`.
    #[must_use]
    pub fn path(&self) -> &'t str {
        self.path_view().as_str()
    }

    /// Returns a borrowed path for inspection without retaining the joined text.
    #[must_use]
    pub fn path_view(&self) -> TagPath<'t> {
        TagPath::new(&self.tree.nodes, self.node())
    }

    /// Returns the number stored as `legacyGUID`, or `None` when the tag has none.
    #[must_use]
    pub fn legacy_id(&self) -> Option<u32> {
        self.node().and_then(|node| node.legacy_id)
    }

    /// Returns how many tags lie between the tag and its root: 0 for a root.
    #[must_use]
    pub fn depth(&self) -> usize {
        self.node().map_or(0, |node| node.depth as usize)
    }

    /// Returns whether the `TagDatabase` lists the tag, directly or below another tag.
    #[must_use]
    pub fn is_listed(&self) -> bool {
        self.node().is_some_and(|node| node.listed)
    }

    /// Returns the parent tag, or `None` for a root.
    #[must_use]
    pub fn parent(&self) -> Option<Self> {
        let index = self.node()?.parent?;
        Some(Self {
            tree: self.tree,
            index,
        })
    }

    /// Returns whether the tag has children.
    #[must_use]
    pub fn has_children(&self) -> bool {
        self.node().is_some_and(|node| !node.children.is_empty())
    }

    /// Returns the children in the order the Tag Record lists them.
    pub fn children(&self) -> impl ExactSizeIterator<Item = Self> + 't {
        let tree = self.tree;
        self.node()
            .map_or(&[] as &[u32], |node| node.children.as_slice())
            .iter()
            .map(move |&index| Self { tree, index })
    }

    /// Returns the tag, its parent, its parent's parent and so on up to the root.
    pub fn ancestors_and_self(&self) -> impl Iterator<Item = Self> + 't {
        std::iter::successors(Some(*self), Self::parent)
    }

    /// Returns every tag below this one, children, their children and so on, in pre-order.
    pub fn descendants(&self) -> impl ExactSizeIterator<Item = Self> + 't {
        let tree = self.tree;
        let (from, to) = self
            .node()
            .map_or((0, 0), |node| (self.index.saturating_add(1), node.end));
        (from..to).map(move |index| Self { tree, index })
    }

    /// Returns whether the tag is `ancestor` or lies below it, at any depth.
    #[must_use]
    pub fn is_within(&self, ancestor: &Self) -> bool {
        std::ptr::eq(self.tree, ancestor.tree)
            && ancestor
                .node()
                .is_some_and(|node| (ancestor.index..node.end).contains(&self.index))
    }
}

impl PartialEq for Tag<'_> {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self.tree, other.tree) && self.index == other.index
    }
}

impl Eq for Tag<'_> {}
