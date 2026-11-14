// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::BankError;
use crate::bank_object_fields::BankHircField;
use crate::hirc_music_switch::{self, BankMusicSwitchNode};

/// One original branch link on the current target's path.
#[derive(Debug)]
#[non_exhaustive]
pub struct BankMusicSwitchBranch {
    /// Physical index of the selecting branch.
    pub node_index: u32,
    /// Original packed child index/count word.
    pub link: BankHircField<u32>,
}

/// One original nonzero target occurrence in decision-tree traversal order.
#[derive(Debug)]
#[non_exhaustive]
pub struct BankMusicSwitchTarget {
    /// Physical index of the target node.
    pub node_index: u32,
    /// Traversal level of the target.
    pub level: u32,
    /// Original target ID.
    pub id: BankHircField<u32>,
}

struct Visit {
    index: u32,
    level: u32,
}

struct Frame {
    next: u32,
    end: u32,
    level: u32,
    branch: Option<BankMusicSwitchBranch>,
}

pub(crate) struct Traversal {
    nodes: u32,
    depth: u32,
    pending: u32,
    visited: u32,
    stack: Vec<Frame>,
}

impl Traversal {
    pub(crate) fn new(nodes: u32, depth: u32) -> Self {
        let mut stack = Vec::new();
        if nodes > 0 {
            stack.push(Frame {
                next: 0,
                end: 1,
                level: 0,
                branch: None,
            });
        }
        Self {
            nodes,
            depth,
            pending: u32::from(nodes > 0),
            visited: 0,
            stack,
        }
    }

    pub(crate) fn next_target<E: From<BankError>>(
        &mut self,
        mut read: impl FnMut(u32) -> Result<BankMusicSwitchNode, E>,
    ) -> Result<Option<BankMusicSwitchTarget>, E> {
        while let Some(visit) = self.next_visit() {
            let node = read(visit.index)?;
            if visit.level < self.depth {
                self.branch(&visit, node.link)?;
            } else if node.link.value != 0 {
                return Ok(Some(BankMusicSwitchTarget {
                    node_index: visit.index,
                    level: visit.level,
                    id: node.link,
                }));
            }
        }
        Ok(None)
    }

    pub(crate) fn route(&self) -> impl Iterator<Item = &BankMusicSwitchBranch> {
        self.stack.iter().filter_map(|frame| frame.branch.as_ref())
    }

    fn next_visit(&mut self) -> Option<Visit> {
        while let Some(frame) = self.stack.last_mut() {
            if frame.next == frame.end {
                self.stack.pop();
                continue;
            }
            let visit = Visit {
                index: frame.next,
                level: frame.level,
            };
            frame.next += 1;
            self.visited += 1;
            self.pending -= 1;
            return Some(visit);
        }
        None
    }

    fn branch(&mut self, visit: &Visit, link: BankHircField<u32>) -> Result<(), BankError> {
        let [a, b, c, d] = link.value.to_le_bytes();
        let first = u32::from(u16::from_le_bytes([a, b]));
        let count = u32::from(u16::from_le_bytes([c, d]));
        let end = first + count;
        if end > self.nodes {
            return Err(hirc_music_switch::invalid(
                "a decision tree node lists children outside the tree",
            ));
        }
        if self.pending + count > self.nodes - self.visited {
            return Err(hirc_music_switch::invalid(
                "a decision tree lists more children than it has nodes left",
            ));
        }
        if count != 0 {
            self.stack.push(Frame {
                next: first,
                end,
                level: visit.level + 1,
                branch: Some(BankMusicSwitchBranch {
                    node_index: visit.index,
                    link,
                }),
            });
            self.pending += count;
        }
        Ok(())
    }
}
