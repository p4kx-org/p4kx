// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use p4kx_dataforge::Record;

use super::Replaced;
use super::layered_worn_free::{self, BoundPort, Holder};
use super::loadout::{is_worn_type, replacement_index};
use super::piece::{Mount, Piece, Source};
use crate::resolved_port_helper;
use crate::resolver::SpecResult;
use crate::{EntityNode, LoadoutResolver, PortConstraints, PortDefinition};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum OverlayMount {
    Original,
    Bound,
    Unresolved(MountProblem),
}

impl OverlayMount {
    pub(super) const fn requires_mount(self) -> bool {
        !matches!(self, Self::Original)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MountProblem {
    NoSlot,
    MissingRecord,
    MissingHolder,
    MissingPort,
    AmbiguousPort,
    IncompatibleItem,
    UnprovenRequirements,
    NoHelper,
    OccupiedPort,
    SelfMount,
    OccupancyUnproven,
}

impl MountProblem {
    pub(super) const fn message(self) -> &'static str {
        match self {
            Self::NoSlot => "the loadout supplies no compatible free attachment port",
            Self::MissingRecord => "the selected item's original Record is unavailable",
            Self::MissingHolder => "the current holder's original Record is unavailable",
            Self::MissingPort => "the current holder does not declare the original port",
            Self::AmbiguousPort => "the loadout supplies competing original or current ports",
            Self::IncompatibleItem => "the current port's item type, subtype or size rules fail",
            Self::UnprovenRequirements => "the current port or item has unproven tag requirements",
            Self::NoHelper => "the current port does not declare a rigid attachment helper",
            Self::OccupiedPort => "the physical port is occupied or claimed more than once",
            Self::SelfMount => "the selected piece would hold itself",
            Self::OccupancyUnproven => {
                "the original occupancy of a compatible port could not be established"
            }
        }
    }
}

enum Decision {
    Original,
    NeedsFreePort,
    Bound(Placement),
    Unresolved(MountProblem),
}

pub(super) struct Placement {
    pub(super) mount: Mount,
    pub(super) label: String,
}

struct Layers<'a> {
    tree: &'a EntityNode,
    base: &'a [Piece],
    over: &'a [Piece],
    replaces: Vec<Option<usize>>,
    located: Vec<(usize, &'a EntityNode)>,
}

impl<'forge> LoadoutResolver<'forge> {
    /// Layers explicit armour over a Loadout and binds rigid pieces to authored ports.
    #[must_use]
    pub fn worn_layered(
        &mut self,
        tree: &EntityNode,
        mut over: Vec<Piece>,
    ) -> (Vec<Piece>, Vec<Replaced>) {
        if over.is_empty() {
            return (self.worn(tree), Vec::new());
        }
        let mut located = Vec::new();
        let mut base = self.worn_located(tree, |index, node| located.push((index, node)));
        let layers = Layers {
            tree,
            located,
            base: &base,
            over: &over,
            replaces: base
                .iter()
                .map(|piece| replacement_index(piece, &over))
                .collect(),
        };
        let decisions = layers.decisions(self);
        for (index, (piece, decision)) in
            base.iter_mut().chain(&mut over).zip(decisions).enumerate()
        {
            piece.overlay_index = Some(index);
            match decision {
                Decision::Original => {}
                Decision::Bound(placed) => {
                    piece.overlay_mount = OverlayMount::Bound;
                    piece.port = Some(placed.label);
                    piece.mount = Some(placed.mount);
                }
                Decision::NeedsFreePort => {
                    piece.overlay_mount = OverlayMount::Unresolved(MountProblem::NoSlot);
                    piece.mount = None;
                }
                Decision::Unresolved(reason) => {
                    piece.overlay_mount = OverlayMount::Unresolved(reason);
                    piece.mount = None;
                }
            }
        }
        Piece::layer(base, over)
    }

    fn worn_spec(&mut self, piece: &Piece) -> Option<SpecResult> {
        let record = self.worn_record(piece)?;
        Some(self.spec_record(&record))
    }

    fn worn_record(&self, piece: &Piece) -> Option<Record<'forge>> {
        let Source::Wearable(wearable) = &piece.source else {
            return None;
        };
        let record = self.entities().forge().record(wearable.record_index?)?;
        (record.guid() == wearable.guid
            && record
                .record_path()
                .is_ok_and(|path| path == wearable.record_path.as_str()))
        .then_some(record)
    }
}

impl Layers<'_> {
    fn decisions(&self, resolver: &mut LoadoutResolver<'_>) -> Vec<Decision> {
        let mut decisions = Vec::with_capacity(self.base.len() + self.over.len());
        for (index, piece) in self.base.iter().enumerate() {
            let needs_refresh = self.replacement(index).is_none()
                && piece
                    .mount
                    .as_ref()
                    .and_then(|mount| mount.holder_index)
                    .is_some_and(|holder| self.replacement(holder).is_some());
            decisions.push(if needs_refresh {
                finish(self.bind(resolver, piece, piece, index))
            } else {
                Decision::Original
            });
        }
        for (index, piece) in self.over.iter().enumerate() {
            if !matches!(&piece.source, Source::Wearable(_)) {
                decisions.push(Decision::Original);
                continue;
            }
            let mut original =
                self.base
                    .iter()
                    .zip(&self.replaces)
                    .filter_map(|(old, candidate)| {
                        (*candidate == Some(index) && old.mount.is_some()).then_some(old)
                    });
            decisions.push(match (original.next(), original.next()) {
                (Some(previous), None) => {
                    finish(self.bind(resolver, previous, piece, self.base.len() + index))
                }
                (None, _) => Decision::NeedsFreePort,
                _ => Decision::Unresolved(MountProblem::AmbiguousPort),
            });
        }
        self.bind_free(resolver, &mut decisions);
        self.reject_duplicate_claims(&mut decisions);
        decisions
    }

    fn replacement(&self, holder: usize) -> Option<usize> {
        self.replaces.get(holder).copied().flatten()
    }

    fn bind(
        &self,
        resolver: &mut LoadoutResolver<'_>,
        previous: &Piece,
        item: &Piece,
        item_index: usize,
    ) -> Result<Placement, MountProblem> {
        let old = previous.mount.as_ref().ok_or(MountProblem::NoSlot)?;
        let label = previous.port().ok_or(MountProblem::NoSlot)?;
        let item_spec = resolver
            .worn_spec(item)
            .ok_or(MountProblem::MissingRecord)?;
        let item = item_spec
            .as_ref()
            .as_ref()
            .map_err(|_| MountProblem::MissingRecord)?;
        let Some(old_holder) = old.holder_index else {
            let definition = self
                .tree
                .ports()
                .iter()
                .filter_map(|port| port.definition())
                .find(|port| port.position() == old.port)
                .ok_or(MountProblem::MissingPort)?;
            let mount = mount_at(None, definition, item)?;
            return Ok(Placement {
                mount,
                label: label.to_owned(),
            });
        };
        let replacement = self.replacement(old_holder);
        let (holder_index, holder) = replacement.map_or_else(
            || (old_holder, self.base.get(old_holder)),
            |index| (self.base.len() + index, self.over.get(index)),
        );
        if holder_index == item_index {
            return Err(MountProblem::SelfMount);
        }
        let holder = holder.ok_or(MountProblem::MissingHolder)?;
        let holder_spec = resolver
            .worn_spec(holder)
            .ok_or(MountProblem::MissingHolder)?;
        let holder_spec = holder_spec
            .as_ref()
            .as_ref()
            .map_err(|_| MountProblem::MissingHolder)?;
        let mut ports = holder_spec.ports.iter().filter(|port| {
            if replacement.is_some() {
                port.name.eq_ignore_ascii_case(label)
            } else {
                port.definition.position() == old.port
            }
        });
        let port = ports.next().ok_or(MountProblem::MissingPort)?;
        if ports.next().is_some() {
            return Err(MountProblem::AmbiguousPort);
        }
        let mount = mount_at(
            Some((holder_index, holder.name().to_owned())),
            &port.definition,
            item,
        )?;
        Ok(Placement {
            mount,
            label: port.name.clone(),
        })
    }

    fn bind_free(&self, resolver: &mut LoadoutResolver<'_>, decisions: &mut [Decision]) {
        if !decisions
            .iter()
            .any(|decision| matches!(decision, Decision::NeedsFreePort))
        {
            return;
        }
        let claims = self.claims(decisions);
        let new_holders: Vec<_> = self
            .over
            .iter()
            .enumerate()
            .filter(|(_, piece)| piece.item_type().is_some_and(is_worn_type))
            .map(|(index, piece)| {
                let tree = resolver
                    .worn_record(piece)
                    .and_then(|record| resolver.resolve(&record).ok());
                (index + self.base.len(), piece.name(), tree)
            })
            .collect();
        let mut holders = vec![Holder {
            identity: None,
            tree: Some(self.tree),
        }];
        holders.extend(self.located.iter().filter_map(|&(index, tree)| {
            let piece = self.base.get(index)?;
            if self.replacement(index).is_some() || !matches!(&piece.source, Source::Wearable(_)) {
                return None;
            }
            Some(Holder {
                identity: Some((index, piece.name())),
                tree: Some(tree),
            })
        }));
        holders.extend(new_holders.iter().map(|(index, label, tree)| Holder {
            identity: Some((*index, *label)),
            tree: tree.as_ref(),
        }));
        for (index, (piece, decision)) in self
            .over
            .iter()
            .zip(decisions.iter_mut().skip(self.base.len()))
            .enumerate()
        {
            if matches!(decision, Decision::NeedsFreePort) {
                let result = resolver
                    .worn_spec(piece)
                    .and_then(|spec| {
                        spec.as_ref().as_ref().ok().map(|spec| {
                            layered_worn_free::select(
                                &holders,
                                &claims,
                                spec,
                                self.base.len() + index,
                            )
                        })
                    })
                    .unwrap_or(Err(MountProblem::MissingRecord));
                *decision = finish(result);
            }
        }
    }

    fn claims(&self, decisions: &[Decision]) -> HashMap<BoundPort, usize> {
        let mut claims = HashMap::<BoundPort, usize>::new();
        for (index, decision) in decisions.iter().enumerate() {
            let mount = match decision {
                Decision::Bound(placed) => Some(&placed.mount),
                Decision::Original if self.replacement(index).is_none() => {
                    self.base.get(index).and_then(|piece| piece.mount.as_ref())
                }
                _ => None,
            };
            if let Some(mount) = mount {
                *claims.entry(BoundPort::of_mount(mount)).or_default() += 1;
            }
        }
        claims
    }

    fn reject_duplicate_claims(&self, decisions: &mut [Decision]) {
        let claims = self.claims(decisions);
        for decision in decisions {
            if let Decision::Bound(placed) = decision
                && claims
                    .get(&BoundPort::of_mount(&placed.mount))
                    .is_some_and(|count| *count > 1)
            {
                *decision = Decision::Unresolved(MountProblem::OccupiedPort);
            }
        }
    }
}

fn finish(result: Result<Placement, MountProblem>) -> Decision {
    match result {
        Ok(mount) => Decision::Bound(mount),
        Err(reason) => Decision::Unresolved(reason),
    }
}

pub(super) fn check_mount_type(
    definition: &PortDefinition,
    item: &crate::spec::EntitySpec,
) -> Result<(), MountProblem> {
    if let Some(constraints) = PortConstraints::of_definition(definition) {
        constraints
            .check(&item.item_facts())
            .map_err(|_| MountProblem::IncompatibleItem)?;
    }
    Ok(())
}

pub(super) fn mount_at(
    holder: Option<(usize, String)>,
    definition: &PortDefinition,
    item: &crate::spec::EntitySpec,
) -> Result<Mount, MountProblem> {
    if definition.requires_attachment_tags || item.requires_attachment_tags {
        return Err(MountProblem::UnprovenRequirements);
    }
    check_mount_type(definition, item)?;
    let resolved = resolved_port_helper::resolve(definition, item.record_index);
    let transform = resolved
        .transform
        .filter(|transform| !transform.helper().is_empty())
        .ok_or(MountProblem::NoHelper)?;
    if definition.is_skin() {
        return Err(MountProblem::NoHelper);
    }
    Ok(Mount::new(
        holder,
        definition.position(),
        transform,
        resolved.diagnostic,
    ))
}
