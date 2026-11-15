// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::audio::SoundVariant;
use crate::audio::hirc_path_source::{HircSelectionChild, HircSelectionOwner};

use super::tree::{Group, Node, PathPosition};

pub(super) struct SelectionTopology {
    owner: Option<PathPosition>,
    children: Vec<PathPosition>,
}

impl SelectionTopology {
    pub(super) fn of(group: &Group) -> Self {
        Self {
            owner: group.source,
            children: group.children.iter().filter_map(Node::source).collect(),
        }
    }

    pub(super) fn resolve<'a>(
        &self,
        variants: &'a [SoundVariant<'_>],
    ) -> Option<SelectionDiagnostic<'a>> {
        let owner = self.owner?.source(variants)?.selection_owner()?;
        let children = self
            .children
            .iter()
            .filter_map(|position| {
                let child = position.source(variants)?.inbound_child()?;
                (child.owner() == owner).then_some(child)
            })
            .collect();
        Some(SelectionDiagnostic { owner, children })
    }
}

pub(super) struct SelectionDiagnostic<'a> {
    pub(super) owner: &'a HircSelectionOwner,
    pub(super) children: Vec<&'a HircSelectionChild>,
}
