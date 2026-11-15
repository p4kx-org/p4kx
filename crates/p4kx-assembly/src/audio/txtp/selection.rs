// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;
use std::ops::Range;
use std::ptr;

use crate::audio::hirc_path_source::{HircRouteSource, HircSelectionChild, HircSelectionOwner};
use crate::audio::{HircPathSource, SoundVariant};

use super::render::RenderedCommand;
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
        commands: &'a [RenderedCommand<'a>],
    ) -> SelectionDiagnostic<'a> {
        let commands = commands.get(self.commands.clone()).unwrap_or_default();
        let mut observed = HashSet::new();
        let routes = commands
            .iter()
            .filter_map(|command| variants.get(command.variant))
            .flat_map(SoundVariant::path_sources)
            .filter_map(HircPathSource::inbound_route)
            .filter(|route| observed.insert(ptr::from_ref(*route)))
            .collect();
        SelectionDiagnostic {
            nodes: self.resolve_nodes(variants),
            commands,
            routes,
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
    pub(super) commands: &'a [RenderedCommand<'a>],
    pub(super) routes: Vec<&'a HircRouteSource>,
}

pub(super) struct SelectionNodes<'a> {
    pub(super) owner: &'a HircSelectionOwner,
    pub(super) children: Vec<&'a HircSelectionChild>,
}
