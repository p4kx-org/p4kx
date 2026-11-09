// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use crate::Guid;
use crate::tags::{
    MAX_TAG_DEPTH, MAX_TAG_NAME_BYTES, MAX_TAG_PATH_BYTES, MAX_TAG_TEXT_BYTES, Node,
    TAG_PATH_SEPARATOR, TagBuildEvent, TagPathEdgeKind, TagPathIncoming, TagProjectionFailure,
    TagRelationIssue, TagTreeIssues,
};

use super::Ticker;
use super::read::{Raw, Root};

pub(super) struct Walk<'a, 'f> {
    pub(super) raws: &'a [Raw<'f>],
    pub(super) positions: &'a HashMap<Guid, usize>,
    pub(super) visited: Vec<bool>,
    pub(super) nodes: Vec<Node>,
    pub(super) roots: Vec<u32>,
    pub(super) text_bytes: usize,
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
                    record: &root.record,
                    field: "tags",
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
                let record = edge.record();
                observe(TagBuildEvent::RelationSkipped {
                    record,
                    field: "tags",
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
            let Some(position) = self.child(source, child.target, observe)? else {
                continue;
            };
            if stack.len() >= MAX_TAG_DEPTH {
                self.issues.too_deep += 1;
                self.leave_out(position, TagProjectionFailure::Depth, observe)?;
                continue;
            }
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
        let end = u32::try_from(self.nodes.len()).unwrap_or(u32::MAX);
        if let Some(Frame { node, .. }) = finished
            && let Some(done) = self.nodes.get_mut(node as usize)
        {
            done.end = end;
        }
    }

    fn child<E>(
        &mut self,
        owner: usize,
        target: Guid,
        observe: &mut impl for<'r> FnMut(TagBuildEvent<'r, 'f>) -> Result<(), E>,
    ) -> Result<Option<usize>, E> {
        let position = self.positions.get(&target).copied();
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
                    record: &raw.record,
                    field: "children",
                    target: Some(target),
                    reason,
                })?;
            }
            Ok(None)
        } else {
            Ok(position)
        }
    }

    const fn projection_limit(
        &self,
        name_len: usize,
        path_len: usize,
        index: Option<u32>,
    ) -> Option<TagProjectionFailure> {
        if name_len > MAX_TAG_NAME_BYTES {
            Some(TagProjectionFailure::NameBytes)
        } else if path_len > MAX_TAG_PATH_BYTES {
            Some(TagProjectionFailure::PathBytes)
        } else if self.text_bytes.saturating_add(path_len) > MAX_TAG_TEXT_BYTES {
            Some(TagProjectionFailure::TextBytes)
        } else if index.is_none() {
            Some(TagProjectionFailure::NodeCount)
        } else {
            None
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
            parent.path.len() + TAG_PATH_SEPARATOR.len_utf8()
        });
        let path_len = prefix + raw.name.len();
        let index = u32::try_from(self.nodes.len()).ok();
        if let Some(reason) = self.projection_limit(raw.name.len(), path_len, index) {
            self.issues.too_long += 1;
            self.leave_out(position, reason, observe)?;
            return Ok(None);
        }
        let (Some(index), Some(visited)) = (index, self.visited.get_mut(position)) else {
            return Ok(None);
        };
        *visited = true;
        let mut path = String::with_capacity(path_len);
        if let Some(parent) = parent_node {
            path.push_str(&parent.path);
            path.push(TAG_PATH_SEPARATOR);
        }
        path.push_str(raw.name);
        let depth = parent_node.map_or(0, |parent| parent.depth.saturating_add(1));
        self.text_bytes += path_len;
        self.nodes.push(Node {
            guid: raw.guid,
            path: path.into(),
            name_start: u32::try_from(prefix).unwrap_or(u32::MAX),
            legacy_id: raw.legacy_id,
            parent,
            children: Vec::new(),
            end: index.saturating_add(1),
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
                path: &node.path,
                depth,
                listed,
            })?;
        }
        Ok(Some(index))
    }

    fn leave_out<E>(
        &mut self,
        position: usize,
        reason: TagProjectionFailure,
        observe: &mut impl for<'r> FnMut(TagBuildEvent<'r, 'f>) -> Result<(), E>,
    ) -> Result<(), E> {
        let mut stack = vec![(position, reason)];
        while let Some((position, reason)) = stack.pop() {
            if self.ticker.stopped() {
                return Ok(());
            }
            match self.visited.get_mut(position) {
                Some(flag) if !*flag => *flag = true,
                _ => continue,
            }
            let Some(raw) = self.raws.get(position) else {
                continue;
            };
            observe(TagBuildEvent::ProjectionFailed {
                record: &raw.record,
                reason,
            })?;
            stack.extend(
                raw.children
                    .iter()
                    .filter_map(|child| self.positions.get(&child.target).copied())
                    .map(|position| (position, TagProjectionFailure::AncestorDiscarded)),
            );
        }
        Ok(())
    }

    pub(super) fn unlisted<E>(
        &mut self,
        observe: &mut impl for<'r> FnMut(TagBuildEvent<'r, 'f>) -> Result<(), E>,
    ) -> Result<(), E> {
        let mut has_parent = vec![false; self.raws.len()];
        for raw in self.raws {
            for child in &raw.children {
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

#[cfg(test)]
#[path = "walk_tests.rs"]
mod tests;
