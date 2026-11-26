// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::convert::Infallible;
use std::iter;
use std::ops::Range;
use std::sync::Arc;

use p4kx_extraction::{DataForge, Guid, TAG_TYPE, TagBuildEvent, TagPathEdgeKind, TagTree, Value};

#[derive(Debug)]
pub(crate) struct ObservedTagTree {
    pub(crate) tree: Arc<TagTree>,
    selected: Vec<SelectedTag>,
}

#[derive(Debug)]
pub(crate) struct SelectedTag {
    pub(crate) record: u32,
    pub(crate) name: Option<Range<u64>>,
    pub(crate) incoming: Option<TagEdge>,
    parent: Option<usize>,
}

#[derive(Debug)]
pub(crate) struct TagEdge {
    pub(crate) owner: u32,
    pub(crate) kind: TagPathEdgeKind,
    pub(crate) ordinal: usize,
}

impl ObservedTagTree {
    pub(super) fn build(forge: &DataForge<'_>, stop: &dyn Fn() -> bool) -> Option<Self> {
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
                TagBuildEvent::PathChosen {
                    record,
                    position,
                    parent_position,
                    incoming,
                    ..
                } if position as usize == selected.len()
                    && parent_position.is_none_or(|parent| parent < position) =>
                {
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
            Ok::<_, Infallible>(())
        });
        let tree = match tree {
            Ok(tree) => tree?,
            Err(never) => match never {},
        };
        Some(Self {
            tree: Arc::new(tree),
            selected,
        })
    }

    pub(crate) fn path(&self, guid: Guid) -> Option<impl Iterator<Item = &SelectedTag>> {
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
