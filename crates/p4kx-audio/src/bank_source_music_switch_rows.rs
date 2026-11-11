// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};

use crate::BankReadError;
use crate::bank_object_fields::{BankHircField, ObjectFields, SeekObjectFields};
use crate::hirc_music_node::{self, BankMusicStinger, MusicList, STINGER_LEN};
use crate::hirc_music_switch::{
    self, BankMusicSwitchGroup, BankMusicSwitchNode, DecisionTree, NODE_LEN,
};
use crate::hirc_music_switch_traversal::{BankMusicSwitchBranch, BankMusicSwitchTarget, Traversal};
use crate::hirc_node::ID_LEN;
use crate::read_at::SeekSource;

use super::SwitchLists;

/// One original physical list row or admitted decision target.
#[derive(Debug)]
#[non_exhaustive]
pub enum BankMusicSwitchRow {
    /// One original child ID.
    Child {
        /// Original list position.
        ordinal: u32,
        /// Original child ID and byte range.
        id: BankHircField<u32>,
    },
    /// One original stinger.
    Stinger {
        /// Original list position.
        ordinal: u32,
        /// Original stinger words and opaque extent.
        value: BankMusicStinger,
    },
    /// One original argument group.
    Group {
        /// Original argument position.
        ordinal: u32,
        /// Original group words.
        value: BankMusicSwitchGroup,
    },
    /// One physical node, whether visited or unvisited.
    Node {
        /// Physical node position.
        ordinal: u32,
        /// Original node words.
        value: BankMusicSwitchNode,
    },
    /// One nonzero target in the checked traversal.
    Target {
        /// Target occurrence position in traversal order.
        ordinal: u32,
        /// Original target and its node position.
        value: BankMusicSwitchTarget,
    },
}

#[derive(Clone, Copy)]
enum Kind {
    Children,
    Stingers,
    Groups,
    Nodes,
    Targets,
}

impl Kind {
    const fn following(self) -> Option<Self> {
        match self {
            Self::Children => Some(Self::Stingers),
            Self::Stingers => Some(Self::Groups),
            Self::Groups => Some(Self::Nodes),
            Self::Nodes => Some(Self::Targets),
            Self::Targets => None,
        }
    }
}

impl SwitchLists {
    fn count(&self, kind: Kind) -> u32 {
        match kind {
            Kind::Children => self.node.children.count,
            Kind::Stingers => self.node.stingers.count,
            Kind::Groups => self.groups.ids.count,
            Kind::Nodes => self.tree.map_or(0, DecisionTree::physical_nodes),
            Kind::Targets => self.targets,
        }
    }

    fn tree(&self) -> Result<DecisionTree, BankReadError> {
        self.tree
            .ok_or_else(|| hirc_music_switch::invalid("the decision tree extent is absent").into())
    }
}

/// Bounded cursor with a stack proportional to the checked target path depth.
pub struct BankMusicSwitchRows<'input, R> {
    input: &'input mut SeekSource<R>,
    stopped: &'input mut bool,
    lists: SwitchLists,
    kind: Option<Kind>,
    next: u32,
    traversal: Option<Traversal>,
    current_target: bool,
}

impl<'input, R> BankMusicSwitchRows<'input, R> {
    pub(super) const fn new(
        input: &'input mut SeekSource<R>,
        stopped: &'input mut bool,
        lists: SwitchLists,
    ) -> Self {
        Self {
            input,
            stopped,
            lists,
            kind: Some(Kind::Children),
            next: 0,
            traversal: None,
            current_target: false,
        }
    }

    /// Returns the exact ancestor branch links of the current target, from the root down.
    pub fn current_target_route(&self) -> impl Iterator<Item = &BankMusicSwitchBranch> {
        self.traversal
            .as_ref()
            .filter(|_| self.current_target)
            .into_iter()
            .flat_map(Traversal::route)
    }
}

impl<R: Read + Seek> BankMusicSwitchRows<'_, R> {
    fn row(&mut self, kind: Kind) -> Result<BankMusicSwitchRow, BankReadError> {
        let ordinal = self.next;
        let node = &self.lists.node;
        Ok(match kind {
            Kind::Children => BankMusicSwitchRow::Child {
                ordinal,
                id: list_reader(self.input, node.children, ordinal, ID_LEN).read_u32()?,
            },
            Kind::Stingers => BankMusicSwitchRow::Stinger {
                ordinal,
                value: hirc_music_node::stinger(&mut list_reader(
                    self.input,
                    node.stingers,
                    ordinal,
                    STINGER_LEN,
                ))?,
            },
            Kind::Groups => BankMusicSwitchRow::Group {
                ordinal,
                value: self.group(ordinal)?,
            },
            Kind::Nodes => BankMusicSwitchRow::Node {
                ordinal,
                value: tree_node(self.input, self.lists.tree()?, ordinal)?,
            },
            Kind::Targets => BankMusicSwitchRow::Target {
                ordinal,
                value: self.target()?,
            },
        })
    }

    fn group(&mut self, ordinal: u32) -> Result<BankMusicSwitchGroup, BankReadError> {
        let groups = &self.lists.groups;
        let id = list_reader(self.input, groups.ids, ordinal, ID_LEN).read_u32()?;
        let type_code = if ordinal < groups.types.count {
            Some(list_reader(self.input, groups.types, ordinal, size_of::<u8>()).read_u8()?)
        } else {
            None
        };
        Ok(BankMusicSwitchGroup { id, type_code })
    }

    fn target(&mut self) -> Result<BankMusicSwitchTarget, BankReadError> {
        let tree = self.lists.tree()?;
        let traversal = self
            .traversal
            .get_or_insert_with(|| Traversal::new(tree.physical_nodes(), tree.depth));
        let target = traversal
            .next_target(|index| tree_node(self.input, tree, index))?
            .ok_or_else(|| hirc_music_switch::invalid("the checked decision target is missing"))?;
        self.current_target = true;
        Ok(target)
    }
}

impl<R: Read + Seek> Iterator for BankMusicSwitchRows<'_, R> {
    type Item = Result<BankMusicSwitchRow, BankReadError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.current_target = false;
        if *self.stopped {
            return None;
        }
        while let Some(kind) = self.kind {
            if self.next == self.lists.count(kind) {
                self.kind = kind.following();
                self.next = 0;
                continue;
            }
            let row = self.row(kind);
            if row.is_ok() {
                self.next += 1;
            } else {
                *self.stopped = true;
            }
            return Some(row);
        }
        None
    }
}

fn list_reader<R>(
    input: &mut SeekSource<R>,
    list: MusicList,
    ordinal: u32,
    width: usize,
) -> SeekObjectFields<'_, R> {
    let position = u64::from(ordinal) * width as u64;
    SeekObjectFields::after_id(input, list.start, list.length, position)
}

pub(super) fn tree_node<R: Read + Seek>(
    input: &mut SeekSource<R>,
    tree: DecisionTree,
    index: u32,
) -> Result<BankMusicSwitchNode, BankReadError> {
    let position = u64::from(index) * u64::from(NODE_LEN);
    let mut reader =
        SeekObjectFields::after_id(input, tree.start, u64::from(tree.byte_len), position);
    hirc_music_switch::node(&mut reader)
}
