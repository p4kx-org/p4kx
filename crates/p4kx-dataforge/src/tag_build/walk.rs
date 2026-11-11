// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use crate::Guid;
use crate::tags::{
    Node, TAG_PATH_SEPARATOR, TagBuildEvent, TagPath, TagPathEdgeKind, TagPathIncoming,
    TagRelationIssue, TagTreeIssues,
};

use super::Ticker;
use super::read::{Child, Raw, Root};

pub(super) struct Walk<'a, 'f> {
    pub(super) raws: &'a [Raw<'f>],
    pub(super) positions: &'a HashMap<Guid, usize>,
    pub(super) visited: Vec<bool>,
    pub(super) nodes: Vec<Node>,
    pub(super) next_node: u32,
    pub(super) roots: Vec<u32>,
    pub(super) names: HashMap<(*const u8, usize), Arc<str>>,
    pub(super) issues: TagTreeIssues,
    pub(super) ticker: Ticker<'a>,
}

struct Frame {
    node: u32,
    raw: usize,
    next: usize,
}

impl<'f> Walk<'_, 'f> {
    pub(super) fn listed_roots<E>(
        &mut self,
        roots: Vec<Root<'f>>,
        observe: &mut impl for<'r> FnMut(TagBuildEvent<'r, 'f>) -> Result<(), E>,
    ) -> Result<(), E> {
        for root in roots {
            if self.ticker.stopped() {
                break;
            }
            if let Some(&position) = self.positions.get(&root.target) {
                self.root(
                    position,
                    true,
                    Some(TagPathIncoming::new(
                        &root.record,
                        TagPathEdgeKind::Root,
                        root.ordinal,
                    )),
                    observe,
                )?;
            } else {
                self.issues.dangling += 1;
                observe(TagBuildEvent::RelationSkipped {
                    incoming: TagPathIncoming::new(
                        &root.record,
                        TagPathEdgeKind::Root,
                        root.ordinal,
                    ),
                    target: Some(root.target),
                    reason: TagRelationIssue::Dangling,
                })?;
            }
        }
        Ok(())
    }

    pub(super) fn root<E>(
        &mut self,
        position: usize,
        listed: bool,
        incoming: Option<TagPathIncoming<'_, 'f>>,
        observe: &mut impl for<'r> FnMut(TagBuildEvent<'r, 'f>) -> Result<(), E>,
    ) -> Result<(), E> {
        if self.visited.get(position).copied().unwrap_or(true) {
            self.issues.repeated += 1;
            if let (Some(edge), Some(raw)) = (incoming, self.raws.get(position)) {
                observe(TagBuildEvent::RelationSkipped {
                    incoming: edge,
                    target: Some(raw.guid),
                    reason: TagRelationIssue::Repeated,
                })?;
            }
            return Ok(());
        }
        let Some(node) = self.enter(position, None, listed, incoming, observe)? else {
            return Ok(());
        };
        self.roots.push(node);
        self.descend(
            Frame {
                node,
                raw: position,
                next: 0,
            },
            listed,
            observe,
        )
    }

    fn descend<E>(
        &mut self,
        root: Frame,
        listed: bool,
        observe: &mut impl for<'r> FnMut(TagBuildEvent<'r, 'f>) -> Result<(), E>,
    ) -> Result<(), E> {
        let mut stack = vec![root];
        while let Some(frame) = stack.last_mut() {
            if self.ticker.stopped() {
                return Ok(());
            }
            let child = self
                .raws
                .get(frame.raw)
                .and_then(|raw| raw.children.get(frame.next))
                .copied();
            let Some(child) = child else {
                self.finish(&mut stack);
                continue;
            };
            frame.next += 1;
            let parent = frame.node;
            let source = frame.raw;
            let Some(position) = self.child(source, child, observe)? else {
                continue;
            };
            let raws = self.raws;
            let incoming = raws.get(source).map(|raw| {
                TagPathIncoming::new(&raw.record, TagPathEdgeKind::Child, child.ordinal)
            });
            if let Some(entered) = self.enter(position, Some(parent), listed, incoming, observe)?
                && let Some(parent) = self.nodes.get_mut(parent as usize)
            {
                parent.children.push(entered);
                stack.push(Frame {
                    node: entered,
                    raw: position,
                    next: 0,
                });
            }
        }
        Ok(())
    }

    fn finish(&mut self, stack: &mut Vec<Frame>) {
        let finished = stack.pop();
        let end = self.next_node;
        if let Some(Frame { node, .. }) = finished
            && let Some(done) = self.nodes.get_mut(node as usize)
        {
            done.end = end;
        }
    }

    fn child<E>(
        &mut self,
        owner: usize,
        child: Child,
        observe: &mut impl for<'r> FnMut(TagBuildEvent<'r, 'f>) -> Result<(), E>,
    ) -> Result<Option<usize>, E> {
        let position = self.positions.get(&child.target).copied();
        let reason = match position {
            None => {
                self.issues.dangling += 1;
                Some(TagRelationIssue::Dangling)
            }
            Some(position) if self.visited.get(position).copied().unwrap_or(true) => {
                self.issues.repeated += 1;
                Some(TagRelationIssue::Repeated)
            }
            Some(_) => None,
        };
        if let Some(reason) = reason {
            if let Some(raw) = self.raws.get(owner) {
                observe(TagBuildEvent::RelationSkipped {
                    incoming: TagPathIncoming::new(
                        &raw.record,
                        TagPathEdgeKind::Child,
                        child.ordinal,
                    ),
                    target: Some(child.target),
                    reason,
                })?;
            }
            Ok(None)
        } else {
            Ok(position)
        }
    }

    fn enter<E>(
        &mut self,
        position: usize,
        parent: Option<u32>,
        listed: bool,
        incoming: Option<TagPathIncoming<'_, 'f>>,
        observe: &mut impl for<'r> FnMut(TagBuildEvent<'r, 'f>) -> Result<(), E>,
    ) -> Result<Option<u32>, E> {
        let Some(raw) = self.raws.get(position) else {
            return Ok(None);
        };
        let parent_node = parent.and_then(|parent| self.nodes.get(parent as usize));
        let prefix = parent_node.map_or(0, |parent| {
            parent.path_bytes + TAG_PATH_SEPARATOR.len_utf8() as u64
        });
        // Each name and the number of Records fit in u32.
        let path_len = prefix + raw.name.len() as u64;
        // One selected node per distinct Record, with the total stored in the UInt32 header count.
        let index = self.next_node;
        let Some(visited) = self.visited.get_mut(position) else {
            return Ok(None);
        };
        *visited = true;
        self.next_node += 1;
        let depth = parent_node.map_or(0, |parent| parent.depth + 1);
        let name = self
            .names
            .entry((raw.name.as_ptr(), raw.name.len()))
            .or_insert_with(|| Arc::from(raw.name));
        self.nodes.push(Node {
            guid: raw.guid,
            name: Arc::clone(name),
            path_bytes: path_len,
            joined: OnceLock::new(),
            legacy_id: raw.legacy_id,
            parent,
            children: Vec::new(),
            end: self.next_node,
            depth,
            listed,
        });
        observe(TagBuildEvent::Projected {
            record: &raw.record,
        })?;
        if let Some(node) = self.nodes.last() {
            observe(TagBuildEvent::PathChosen {
                record: &raw.record,
                position: index,
                parent_position: parent,
                incoming,
                path: TagPath::new(&self.nodes, Some(node)),
                depth,
                listed,
            })?;
        }
        Ok(Some(index))
    }

    pub(super) fn unlisted<E>(
        &mut self,
        observe: &mut impl for<'r> FnMut(TagBuildEvent<'r, 'f>) -> Result<(), E>,
    ) -> Result<(), E> {
        let mut has_parent = vec![false; self.raws.len()];
        for raw in self.raws {
            for child in &raw.children {
                if self.ticker.stopped() {
                    return Ok(());
                }
                if let Some(&position) = self.positions.get(&child.target)
                    && let Some(flag) = has_parent.get_mut(position)
                {
                    *flag = true;
                }
            }
        }
        for orphans_first in [true, false] {
            for position in 0..self.raws.len() {
                if self.ticker.stopped() {
                    return Ok(());
                }
                let unvisited = self.visited.get(position) == Some(&false);
                let parentless = has_parent.get(position) == Some(&false);
                if unvisited && (parentless || !orphans_first) {
                    self.root(position, false, None, observe)?;
                }
            }
        }
        Ok(())
    }
}
