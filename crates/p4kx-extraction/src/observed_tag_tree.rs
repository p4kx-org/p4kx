// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::iter;
use std::ops::Range;
use std::sync::Arc;

use p4kx_dataforge::{
    DataForge, Guid, RecordJsonBytes, RecordJsonSource, TAG_TYPE, TagBuildEvent, TagPathEdgeKind,
    TagResolver, TagTree, Value,
};

use crate::{
    DataForgeEntry, DataForgeOrigin, InputScope, RecordJsonLocation, SourceEvidenceError,
    SourceInput, SourceLocation, SourceScope, SourceUse, SourceUseRole, SourceUses,
};

/// Resolves tag paths and retains the selected source steps from the same input.
pub struct ObservedTagTree {
    input: Arc<DataForgeEntry>,
    tree: Arc<TagTree>,
    selected: Vec<SelectedTag>,
}

struct SelectedTag {
    record: u32,
    name: Option<Range<u64>>,
    incoming: Option<TagEdge>,
    parent: Option<usize>,
}

struct TagEdge {
    owner: u32,
    kind: TagPathEdgeKind,
    ordinal: usize,
}

impl ObservedTagTree {
    /// Builds one observed tree or returns `None` if the build is cancelled.
    ///
    /// # Errors
    /// Returns an error if the DataForge was not opened over this input.
    pub fn build(
        input: Arc<DataForgeEntry>,
        forge: &DataForge<'_>,
        stop: &dyn Fn() -> bool,
    ) -> Result<Option<Self>, SourceEvidenceError> {
        if input.origin_of(forge).is_none() {
            return Err(SourceEvidenceError::ForeignEvidence);
        }
        let mut names = BTreeMap::new();
        let mut selected = Vec::new();
        let tree = TagTree::build_observed(forge, stop, &mut |event| {
            match event {
                TagBuildEvent::CanonicalRead {
                    record,
                    outcome: Ok(tree),
                } if tree.type_name() == TAG_TYPE => {
                    if let Some(Value::String(name)) = tree.field("tagName")
                        && let Some(range) = forge.original_text_span(name)
                    {
                        names.insert(record.index(), range);
                    }
                }
                TagBuildEvent::DuplicateSkipped { record } => {
                    names.remove(&record.index());
                }
                TagBuildEvent::PathChosen {
                    record,
                    position,
                    parent_position,
                    incoming,
                    ..
                } => {
                    if position as usize != selected.len()
                        || parent_position.is_some_and(|parent| parent >= position)
                    {
                        return Err(SourceEvidenceError::ForeignEvidence);
                    }
                    selected.push(SelectedTag {
                        record: record.index(),
                        name: names.remove(&record.index()),
                        incoming: incoming.map(|edge| TagEdge {
                            owner: edge.record().index(),
                            kind: edge.kind(),
                            ordinal: edge.ordinal(),
                        }),
                        parent: parent_position.map(|parent| parent as usize),
                    });
                }
                _ => {}
            }
            Ok(())
        })?;
        let Some(tree) = tree else {
            return Ok(None);
        };
        if selected.len() != tree.len() {
            return Err(SourceEvidenceError::ForeignEvidence);
        }
        Ok(Some(Self {
            input,
            tree: Arc::new(tree),
            selected,
        }))
    }

    /// Returns whether the DataForge was opened over the tree's exact input.
    #[must_use]
    pub fn is_from(&self, forge: &DataForge<'_>) -> bool {
        self.input.origin_of(forge).is_some()
    }

    /// Shares the tree that resolves the observed paths.
    #[must_use]
    pub fn shared_tree(&self) -> Arc<TagTree> {
        Arc::clone(&self.tree)
    }

    /// Binds selected tag path coordinates to the exact DataForge payload.
    ///
    /// # Errors
    /// Returns an error for a different input or an unavailable evidence registry.
    pub fn bind_paths(
        &self,
        forge: &DataForge<'_>,
        uses: &SourceUses,
    ) -> Result<TagPathOrigins<'_>, SourceEvidenceError> {
        let origin = self
            .input
            .origin_of(forge)
            .ok_or(SourceEvidenceError::ForeignEvidence)?;
        let (source, complete) = match origin {
            DataForgeOrigin::Archive(entry) => (
                SourceInput::Archive(entry),
                entry.ancestors().all(|step| step.is_some()),
            ),
            DataForgeOrigin::File(file) => (SourceInput::File(file), true),
            DataForgeOrigin::Unknown => (SourceInput::Unknown, false),
        };
        Ok(TagPathOrigins {
            tags: self,
            scope: uses.observe(source, InputScope::Payload)?,
            complete,
        })
    }

    pub(crate) fn observed_locations(
        &self,
        forge: &DataForge<'_>,
        guid: Guid,
    ) -> Result<Option<impl Iterator<Item = Option<RecordJsonLocation>>>, SourceEvidenceError> {
        if !self.is_from(forge) {
            return Err(SourceEvidenceError::ForeignEvidence);
        }
        Ok(self.locations(guid))
    }

    fn locations(&self, guid: Guid) -> Option<impl Iterator<Item = Option<RecordJsonLocation>>> {
        self.path(guid)
            .map(|path| path.flat_map(SelectedTag::locations))
    }

    fn path(&self, guid: Guid) -> Option<impl Iterator<Item = &SelectedTag>> {
        let tag = self.tree.get(guid)?;
        let selected = self.selected.get(tag.position())?;
        if tag.record_index() != Some(selected.record) {
            return None;
        }
        Some(iter::successors(Some(selected), |current| {
            self.selected.get(current.parent?)
        }))
    }
}

impl SelectedTag {
    fn locations(&self) -> impl Iterator<Item = Option<RecordJsonLocation>> {
        let name = self.name.as_ref().and_then(|bytes| {
            RecordJsonLocation::of(RecordJsonSource::Bytes {
                start: bytes.start,
                end: bytes.end,
                kind: RecordJsonBytes::Text,
            })
        });
        let authored = name.as_ref().map(|_| RecordJsonLocation {
            location: SourceLocation::DataForge {
                record_index: self.record,
                field: "tagName".into(),
            },
            role: SourceUseRole::Conversion,
        });
        let edge = self.incoming.as_ref().map(|edge| RecordJsonLocation {
            location: SourceLocation::Subresource {
                namespace: "dataforgeTagRelations".into(),
                indices: vec![u64::from(edge.owner), edge.ordinal as u64],
                field: edge.kind.field().into(),
            },
            role: SourceUseRole::Selector,
        });
        authored
            .into_iter()
            .map(Some)
            .chain(iter::once(name))
            .chain(edge.map(Some))
    }
}

impl TagResolver for ObservedTagTree {
    fn tag_path(&self, guid: Guid) -> Option<&str> {
        self.tree.tag_path(guid)
    }

    fn tag_path_text(&self, guid: Guid) -> Option<Cow<'_, str>> {
        self.tree.tag_path_text(guid)
    }
}

/// The selected tag path contributors bound to their physical input.
pub struct TagPathOrigins<'a> {
    tags: &'a ObservedTagTree,
    scope: SourceScope,
    complete: bool,
}

impl TagPathOrigins<'_> {
    /// Appends the selected name and edge sources and reports their completeness.
    ///
    /// # Errors
    /// Returns an error when the evidence registry cannot record a source use.
    pub fn append(
        &self,
        guid: Guid,
        contributors: &mut Vec<SourceUse>,
    ) -> Result<bool, SourceEvidenceError> {
        let Some(path) = self.tags.locations(guid) else {
            return Ok(false);
        };
        let mut complete = self.complete;
        for site in path {
            if let Some(site) = site {
                contributors.push(self.scope.at(site.location, site.role)?);
            } else {
                complete = false;
            }
        }
        Ok(complete)
    }
}
