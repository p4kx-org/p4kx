// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use crate::audio::hirc_path_source::{HircSelectionChild, HircSelectionOwner};
use crate::audio::{ExportedWemOrigin, SoundVariant};

use super::tree::{Group, Node, PathPosition};

pub(super) struct SelectionTopology {
    owner: Option<PathPosition>,
    children: Vec<PathPosition>,
    commands: Range<usize>,
}

impl SelectionTopology {
    pub(super) fn of(group: &Group, commands: Range<usize>) -> Self {
        Self {
            owner: group.source,
            children: group.children.iter().filter_map(Node::source).collect(),
            commands,
        }
    }

    pub(super) fn resolve<'a>(
        &self,
        variants: &'a [SoundVariant<'_>],
        media: &'a [Option<&'a ExportedWemOrigin>],
    ) -> SelectionDiagnostic<'a> {
        SelectionDiagnostic {
            nodes: self.resolve_nodes(variants),
            media: media.get(self.commands.clone()).unwrap_or_default(),
        }
    }

    fn resolve_nodes<'a>(&self, variants: &'a [SoundVariant<'_>]) -> Option<SelectionNodes<'a>> {
        let owner = self.owner?.source(variants)?.selection_owner()?;
        let children = self
            .children
            .iter()
            .filter_map(|position| {
                let child = position.source(variants)?.inbound_child()?;
                (child.owner() == owner).then_some(child)
            })
            .collect();
        Some(SelectionNodes { owner, children })
    }
}

pub(super) struct SelectionDiagnostic<'a> {
    pub(super) nodes: Option<SelectionNodes<'a>>,
    pub(super) media: &'a [Option<&'a ExportedWemOrigin>],
}

pub(super) struct SelectionNodes<'a> {
    pub(super) owner: &'a HircSelectionOwner,
    pub(super) children: Vec<&'a HircSelectionChild>,
}
