// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use super::layered_worn::{MountProblem, Placement, check_mount_type, mount_at};
use super::piece::Mount;
use crate::spec::EntitySpec;
use crate::{Attachment, EntityNode, PortDefinitionPosition, PortNode, PortVisibility};

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct BoundPort {
    holder: Option<usize>,
    position: PortDefinitionPosition,
}

impl BoundPort {
    pub(super) const fn of_mount(mount: &Mount) -> Self {
        Self {
            holder: mount.holder_index,
            position: mount.port,
        }
    }
}

pub(super) struct Holder<'node> {
    pub(super) identity: Option<(usize, &'node str)>,
    pub(super) tree: Option<&'node EntityNode>,
}

struct Candidate<'node> {
    identity: Option<(usize, &'node str)>,
    port: &'node PortNode,
}

#[derive(Default)]
struct Search<'node> {
    candidate: Option<Candidate<'node>>,
    unknown: Option<MountProblem>,
    occupied: bool,
}

pub(super) fn select(
    holders: &[Holder<'_>],
    claims: &HashMap<BoundPort, usize>,
    item: &EntitySpec,
    item_index: usize,
) -> Result<Placement, MountProblem> {
    let mut search = Search::default();
    for holder in holders {
        if holder
            .identity
            .is_some_and(|(index, _)| index == item_index)
        {
            continue;
        }
        let Some(tree) = holder.tree else {
            search.unknown = Some(MountProblem::OccupancyUnproven);
            continue;
        };
        if !tree.is_expanded() {
            search.unknown = Some(MountProblem::OccupancyUnproven);
            continue;
        }
        if !tree.issues().is_empty() {
            search.unknown = Some(MountProblem::OccupancyUnproven);
            continue;
        }
        for port in tree.ports() {
            search.visit(holder, port, item, claims)?;
        }
    }
    search.finish(item)
}

impl<'node> Search<'node> {
    fn visit(
        &mut self,
        holder: &Holder<'node>,
        port: &'node PortNode,
        item: &EntitySpec,
        claims: &HashMap<BoundPort, usize>,
    ) -> Result<(), MountProblem> {
        let Some(definition) = port.definition() else {
            return Ok(());
        };
        if port.visibility() == PortVisibility::Invisible
            || holder.tree.and_then(EntityNode::record_index)
                != Some(definition.position().record_index())
            || definition.is_skin()
            || definition
                .transform()
                .is_none_or(|transform| transform.helper().is_empty())
            || check_mount_type(definition, item).is_err()
        {
            return Ok(());
        }
        let slot = BoundPort {
            holder: holder.identity.map(|(index, _)| index),
            position: definition.position(),
        };
        if claims.contains_key(&slot) || !port.nested().is_empty() {
            self.occupied = true;
            return Ok(());
        }
        match port.attachment() {
            Attachment::Entity(_) => self.occupied = true,
            Attachment::Unresolved(_) => self.unknown = Some(MountProblem::OccupancyUnproven),
            Attachment::Empty
                if definition.requires_attachment_tags || item.requires_attachment_tags =>
            {
                self.unknown = Some(MountProblem::UnprovenRequirements);
            }
            Attachment::Empty => {
                if self.candidate.is_some() {
                    return Err(MountProblem::AmbiguousPort);
                }
                self.candidate = Some(Candidate {
                    identity: holder.identity,
                    port,
                });
            }
        }
        Ok(())
    }

    fn finish(self, item: &EntitySpec) -> Result<Placement, MountProblem> {
        if let Some(reason) = self.unknown {
            return Err(reason);
        }
        let candidate = self.candidate.ok_or(if self.occupied {
            MountProblem::OccupiedPort
        } else {
            MountProblem::NoSlot
        })?;
        let definition = candidate
            .port
            .definition()
            .ok_or(MountProblem::MissingPort)?;
        let holder = candidate
            .identity
            .map(|(index, label)| (index, label.to_owned()));
        let mount = mount_at(holder, definition, item)?;
        Ok(Placement {
            mount,
            label: candidate.port.name().to_owned(),
        })
    }
}
